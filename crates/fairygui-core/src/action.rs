//! 控制器动作。
//!
//! 动作在页面切换时执行，目前有两种：播放过渡动画，以及把另一个对象的
//! 控制器切到指定页面。两者的前置部分相同——先记录「从哪些页面」与
//! 「切到哪些页面」，后面的专属字段再各自不同。

use crate::binary::{ByteBuffer, StringTable};
use crate::error::Result;

/// 一个控制器动作。
#[derive(Debug, Clone, PartialEq)]
pub enum ControllerAction {
    /// 播放过渡动画。
    PlayTransition {
        /// 切换前的页面 ID。为空表示任意来源页。
        from_pages: Vec<String>,
        /// 切换后的页面 ID。为空表示任意目标页。
        to_pages: Vec<String>,
        /// 过渡名称。
        transition_name: String,
        /// 播放次数，`0` 表示无限。
        play_times: i32,
        /// 播放前的延迟（秒）。
        delay: f32,
        /// 离开该页面时是否停止播放。
        stop_on_exit: bool,
    },
    /// 切换另一个对象的控制器页面。
    ChangePage {
        /// 切换前的页面 ID。为空表示任意来源页。
        from_pages: Vec<String>,
        /// 切换后的页面 ID。为空表示任意目标页。
        to_pages: Vec<String>,
        /// 目标直接子对象的导出实例 ID；空串表示控制器自身所属的容器。
        object_id: String,
        /// 目标控制器名。
        controller_name: String,
        /// 目标页面 ID，或 `~1`（转发序号）/ `~2`（转发名称）。
        target_page: String,
    },
}

impl ControllerAction {
    /// 读取一个动作。
    ///
    /// `buffer` 的位置应当紧跟在动作类型字节之后。未知类型返回 `None`，
    /// 长度对齐由调用方通过块偏移保证。
    pub(crate) fn setup(
        buffer: &mut ByteBuffer<'_>,
        table: &StringTable,
        action_type: u8,
    ) -> Result<Option<Self>> {
        let from_pages = read_pages(buffer, table)?;
        let to_pages = read_pages(buffer, table)?;

        Ok(match action_type {
            0 => Some(Self::PlayTransition {
                from_pages,
                to_pages,
                transition_name: buffer.read_s(table)?.unwrap_or_default(),
                play_times: buffer.read_i32()?,
                delay: buffer.read_f32()?,
                stop_on_exit: buffer.read_bool()?,
            }),
            1 => Some(Self::ChangePage {
                from_pages,
                to_pages,
                object_id: buffer.read_s(table)?.unwrap_or_default(),
                controller_name: buffer.read_s(table)?.unwrap_or_default(),
                target_page: buffer.read_s(table)?.unwrap_or_default(),
            }),
            _ => None,
        })
    }

    /// 该动作是否由指定页面触发。
    pub fn triggers_from(&self, page: &str) -> bool {
        let from_pages = match self {
            Self::PlayTransition { from_pages, .. } | Self::ChangePage { from_pages, .. } => {
                from_pages
            }
        };
        from_pages.is_empty() || from_pages.iter().any(|candidate| candidate == page)
    }

    /// 同时匹配切换前后的页面 ID；未选中页面只匹配空的筛选列表。
    pub fn matches_pages(&self, previous: Option<&str>, current: Option<&str>) -> bool {
        let (from_pages, to_pages) = match self {
            Self::PlayTransition {
                from_pages,
                to_pages,
                ..
            }
            | Self::ChangePage {
                from_pages,
                to_pages,
                ..
            } => (from_pages, to_pages),
        };
        let matches = |pages: &[String], page: Option<&str>| {
            pages.is_empty() || page.is_some_and(|page| pages.iter().any(|id| id == page))
        };
        matches(from_pages, previous) && matches(to_pages, current)
    }
}

fn read_pages(buffer: &mut ByteBuffer<'_>, table: &StringTable) -> Result<Vec<String>> {
    let count = buffer.read_i16()?.max(0) as usize;
    let mut pages = Vec::with_capacity(count);
    for _ in 0..count {
        pages.push(buffer.read_s(table)?.unwrap_or_default());
    }
    Ok(pages)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table_with(entries: &[&str]) -> StringTable {
        let mut table = StringTable::new();
        for entry in entries {
            table.push(Some((*entry).to_string()));
        }
        table
    }

    #[test]
    fn play_transition_reads_its_own_fields() {
        // 一个页面的名字：索引 0；其余字符串同样用索引 0。
        let table = table_with(&["page"]);
        let mut data = Vec::new();
        data.extend_from_slice(&1i16.to_be_bytes()); // fromPage 个数
        data.extend_from_slice(&0u16.to_be_bytes()); // fromPage[0]
        data.extend_from_slice(&1i16.to_be_bytes()); // toPage 个数
        data.extend_from_slice(&0u16.to_be_bytes()); // toPage[0]
        data.extend_from_slice(&0u16.to_be_bytes()); // transitionName
        data.extend_from_slice(&3i32.to_be_bytes()); // playTimes
        data.extend_from_slice(&0.5f32.to_be_bytes()); // delay
        data.push(1); // stopOnExit

        let mut buffer = ByteBuffer::new(&data);
        let action = ControllerAction::setup(&mut buffer, &table, 0)
            .expect("setup")
            .expect("known type");

        match action {
            ControllerAction::PlayTransition {
                from_pages,
                to_pages,
                transition_name,
                play_times,
                delay,
                stop_on_exit,
            } => {
                assert_eq!(from_pages, vec!["page".to_string()]);
                assert_eq!(to_pages, vec!["page".to_string()]);
                assert_eq!(transition_name, "page");
                assert_eq!(play_times, 3);
                assert!((delay - 0.5).abs() < 1e-6);
                assert!(stop_on_exit);
            }
            other => panic!("类型不符：{other:?}"),
        }
    }

    #[test]
    fn change_page_reads_its_own_fields() {
        let table = table_with(&["page"]);
        let mut data = Vec::new();
        data.extend_from_slice(&1i16.to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&0i16.to_be_bytes()); // toPage 个数为 0
        data.extend_from_slice(&0u16.to_be_bytes()); // objectId
        data.extend_from_slice(&0u16.to_be_bytes()); // controllerName
        data.extend_from_slice(&0u16.to_be_bytes()); // targetPage

        let mut buffer = ByteBuffer::new(&data);
        let action = ControllerAction::setup(&mut buffer, &table, 1)
            .expect("setup")
            .expect("known type");

        match action {
            ControllerAction::ChangePage {
                from_pages,
                to_pages,
                ..
            } => {
                assert_eq!(from_pages.len(), 1);
                assert!(to_pages.is_empty());
            }
            other => panic!("类型不符：{other:?}"),
        }
    }

    #[test]
    fn unknown_action_type_is_skipped() {
        let table = StringTable::new();
        let mut data = Vec::new();
        data.extend_from_slice(&0i16.to_be_bytes());
        data.extend_from_slice(&0i16.to_be_bytes());

        let mut buffer = ByteBuffer::new(&data);
        let action = ControllerAction::setup(&mut buffer, &table, 9).expect("setup");
        assert!(action.is_none(), "未知类型应当返回空");
    }

    #[test]
    fn trigger_matching_handles_empty_and_explicit_pages() {
        let any_page = ControllerAction::ChangePage {
            from_pages: Vec::new(),
            to_pages: Vec::new(),
            object_id: String::new(),
            controller_name: String::new(),
            target_page: String::new(),
        };
        assert!(any_page.triggers_from("anything"), "空列表表示任意页面");

        let specific = ControllerAction::ChangePage {
            from_pages: vec!["a".to_string()],
            to_pages: Vec::new(),
            object_id: String::new(),
            controller_name: String::new(),
            target_page: String::new(),
        };
        assert!(specific.triggers_from("a"));
        assert!(!specific.triggers_from("b"));
    }
}
