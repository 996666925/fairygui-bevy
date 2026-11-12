//! 序列帧播放。
//!
//! 帧数据来自包内条目，播放状态挂在实体上。每帧只更新纹理的子区域，
//! **不重建实体**——这正是序列帧能低成本播放的原因。
//!
//! 普通更新保留官方的分段循环与完成状态；DeltaTime 补偿单独处理往返和跳帧。
//! `repeat_delay` 在重复经过第 0 帧时计入，帧计时在内部使用毫秒。

use bevy::prelude::*;

use fairygui_core::object::{AnimationCommand, AnimationPlaySettings};
use fairygui_core::package::{FrameTexture, MovieClipData};

#[cfg(test)]
#[path = "movie_reference_tests.rs"]
mod reference_tests;

/// 实体的序列帧播放状态。
#[derive(Component, Debug, Clone)]
pub struct MovieClipPlayer {
    frames: Vec<fairygui_core::package::MovieFrame>,
    atlases: std::collections::HashMap<usize, Handle<Image>>,
    /// 帧间隔（毫秒），与官方计时单位一致。
    interval: f64,
    /// 是否往复播放。
    swing: bool,
    /// 每轮循环前的延迟（毫秒）。
    repeat_delay: f64,
    /// 当前帧。
    frame: usize,
    /// 往复播放时的方向。
    forward: bool,
    /// 累积但不足一帧的时间。
    time: f64,
    repeated: bool,
    /// 是否正在播放。静止的序列帧仍会显示当前帧，但不推进。
    playing: bool,
    time_scale: f32,
    start: usize,
    end: usize,
    remaining_times: i32,
    end_at: usize,
    status: PlaybackStatus,
    completion_pending: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlaybackStatus {
    Playing,
    NextLoop,
    Ending,
    Ended,
}

impl MovieClipPlayer {
    /// 由帧数据创建播放状态。
    ///
    /// `start_frame` 为负时从第 0 帧开始。帧间隔保留包内的毫秒单位。
    pub fn new(clip: &MovieClipData, start_frame: i32, playing: bool) -> Option<Self> {
        if clip.frames.is_empty() {
            return None;
        }

        let frame = if start_frame < 0 {
            0
        } else {
            (start_frame as usize).min(clip.frames.len() - 1)
        };

        Some(Self {
            frames: clip.frames.clone(),
            atlases: default(),
            interval: f64::from(clip.interval.max(0)),
            swing: clip.swing,
            repeat_delay: f64::from(clip.repeat_delay.max(0)),
            frame,
            forward: true,
            time: 0.0,
            repeated: false,
            // 静止的序列帧不推进，但当前帧仍会被应用。
            playing,
            time_scale: 1.0,
            start: 0,
            end: clip.frames.len() - 1,
            remaining_times: 0,
            end_at: clip.frames.len() - 1,
            status: PlaybackStatus::Playing,
            completion_pending: false,
        })
    }

    /// 当前帧的纹理来源。
    pub fn current(&self) -> Option<&FrameTexture> {
        self.frames.get(self.frame)?.texture.as_ref()
    }

    /// 当前帧序号。
    pub fn frame(&self) -> usize {
        self.frame
    }

    /// 帧数量。
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// 普通逐帧更新：先将输入夹到 100ms，再乘速度，每次最多前进一帧。
    /// 剩余时间最多保留一个基本帧间隔；批量补偿使用 advance_time。
    pub fn advance(&mut self, dt: f32) -> bool {
        if !self.playing || self.status == PlaybackStatus::Ended || !dt.is_finite() || dt < 0.0 {
            return false;
        }

        let before = self.frame;
        self.time += (f64::from(dt) * 1000.0).min(100.0) * f64::from(self.time_scale);
        let duration = self.frame_duration();
        if self.time < duration {
            return false;
        }
        self.time = (self.time - duration).min(self.interval);
        self.step();
        match self.status {
            PlaybackStatus::NextLoop => {
                self.frame = self.start;
                self.time = 0.0;
                self.status = PlaybackStatus::Playing;
            }
            PlaybackStatus::Ending => {
                self.frame = self.end_at;
                self.time = 0.0;
                self.status = PlaybackStatus::Ended;
                self.completion_pending = true;
            }
            PlaybackStatus::Playing if self.frame == self.end => {
                if self.remaining_times > 0 {
                    self.remaining_times -= 1;
                    self.status = if self.remaining_times == 0 {
                        PlaybackStatus::Ending
                    } else {
                        PlaybackStatus::NextLoop
                    };
                } else {
                    self.status = PlaybackStatus::NextLoop;
                }
            }
            _ => {}
        }

        self.frame != before
    }

    fn frame_duration(&self) -> f64 {
        self.interval
            + f64::from(self.frames[self.frame].add_delay.max(0))
            + if self.frame == 0 && self.repeated {
                self.repeat_delay
            } else {
                0.0
            }
    }

    /// 与官方 setPlaySettings 一样，不会修改 Playing、方向、循环延迟状态或速度。
    pub fn set_play_settings(&mut self, settings: AnimationPlaySettings) -> bool {
        if !settings.is_valid() {
            return false;
        }
        let last = self.frames.len() - 1;
        self.start = (settings.start as usize).min(last);
        self.end = if settings.end < 0 {
            last
        } else {
            (settings.end as usize).min(last)
        };
        self.end_at = if settings.end_at < 0 {
            self.end
        } else {
            (settings.end_at as usize).min(last)
        };
        self.remaining_times = settings.times;
        self.status = PlaybackStatus::Playing;
        self.completion_pending = false;
        self.goto(self.start as i32);
        true
    }

    pub fn is_ended(&self) -> bool {
        self.status == PlaybackStatus::Ended
    }

    /// 消费本次有限播放的完成通知；设置分段播放后可再次产生通知。
    pub fn take_completion(&mut self) -> bool {
        std::mem::take(&mut self.completion_pending)
    }

    /// 回到首帧；与官方 rewind 一样，不重置已结束状态或分段播放配置。
    pub fn rewind(&mut self) {
        self.frame = 0;
        self.time = 0.0;
        self.forward = true;
        self.repeated = false;
    }

    /// 只同步播放位置、方向与帧内时间，保留接收方的 Playing/速度/分段配置。
    pub fn sync_status(&mut self, source: &Self) {
        self.frame = source.frame.min(self.frames.len() - 1);
        self.time = source.time;
        self.forward = source.forward;
        self.repeated = source.repeated;
    }

    /// FairyGUI DeltaTime: advance whole frames without Playing/timeScale and
    /// discard fractional time. Cycles are skipped for large interval seeks.
    pub fn advance_time(&mut self, seconds: f32) -> bool {
        if !seconds.is_finite() || seconds < 0.0 {
            return false;
        }
        let before = self.frame;
        let mut remaining = f64::from(seconds) * 1000.0;
        let initial = (self.frame, self.forward);
        let backup = remaining;
        let mut native_cycle = false;
        let mut seen = std::collections::HashMap::new();
        loop {
            let state = (self.frame, self.forward, self.repeated);
            if !native_cycle && let Some(previous) = seen.insert(state, remaining) {
                let cycle = previous - remaining;
                if cycle <= 0.0 {
                    break;
                } // malformed all-zero-duration cycle
                remaining %= cycle;
                seen.clear();
            }
            let duration = self.frame_duration();
            if remaining < duration {
                break;
            }
            remaining -= duration;
            self.step();
            if (self.frame, self.forward) == initial {
                // Match the reference's first-return cycle, including its
                // initial-loop treatment of repeatDelay. Using a steady-state
                // cycle instead changes official swing/catch-up results.
                let round = backup - remaining;
                if round <= 0.0 {
                    break;
                }
                remaining %= round;
                native_cycle = true;
            }
        }
        self.time = 0.0;
        self.frame != before
    }

    fn step(&mut self) {
        let last = self.frames.len() - 1;

        if self.swing {
            if self.forward {
                if self.frame >= last {
                    self.forward = false;
                    self.repeated = true;
                    self.frame = last.saturating_sub(1);
                } else {
                    self.frame += 1;
                }
            } else if self.frame == 0 {
                self.forward = true;
                self.repeated = true;
            } else {
                self.frame -= 1;
                if self.frame == 0 {
                    self.forward = true;
                    self.repeated = true;
                }
            }
            return;
        }

        if self.frame >= last {
            self.frame = 0;
            self.repeated = true;
        } else {
            self.frame += 1;
        }
    }
}

impl MovieClipPlayer {
    pub fn time_scale(&self) -> f32 {
        self.time_scale
    }
    pub fn set_time_scale(&mut self, scale: f32) -> bool {
        if !scale.is_finite() || scale < 0.0 || self.time_scale == scale {
            return false;
        }
        self.time_scale = scale;
        true
    }
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn bind_atlas(&mut self, index: usize, image: Handle<Image>) {
        self.atlases.insert(index, image);
    }

    pub fn apply_frame(&self, image: &mut ImageNode) {
        if let Some((atlas, rect)) = self.render_frame() {
            if let Some(atlas) = atlas {
                image.image = atlas.clone();
            }
            image.rect = Some(rect);
        }
    }

    pub(crate) fn render_frame(&self) -> Option<(Option<&Handle<Image>>, Rect)> {
        let frame = self.current()?;
        Some((
            self.atlases.get(&frame.atlas_item),
            Rect::new(
                frame.rect.x,
                frame.rect.y,
                frame.rect.x + frame.rect.width,
                frame.rect.y + frame.rect.height,
            ),
        ))
    }

    /// 开始播放。
    pub fn play(&mut self) {
        self.playing = true;
    }

    /// 暂停播放。
    pub fn stop(&mut self) {
        self.playing = false;
    }

    /// 跳转到指定帧。
    pub fn goto(&mut self, frame: i32) {
        if self.frames.is_empty() || self.frame as i32 == frame {
            return;
        }
        let frame = if frame < 0 {
            0
        } else {
            (frame as usize).min(self.frames.len() - 1)
        };
        self.frame = frame;
        self.time = 0.0;
    }
}

/// 每帧推进序列帧，并按当前帧更新纹理子区域。
pub fn advance_movie_clips(
    mut query: Query<(
        &crate::GObjectRef,
        &mut MovieClipPlayer,
        Option<&mut ImageNode>,
    )>,
    time: Res<Time>,
    mut runtime: NonSendMut<crate::GuiRuntime>,
) {
    let dt = time.delta_secs();

    for (object, mut player, image) in &mut query {
        let Some((playing, frame)) = runtime.store().animation_state(object.0) else {
            continue;
        };
        if let Some(scale) = runtime.store().animation_time_scale(object.0)
            && player.time_scale() != scale
        {
            player.set_time_scale(scale);
        }
        if playing {
            player.play();
        } else {
            player.stop();
        }
        let commands = runtime.store_mut().take_animation_commands(object.0);
        if !commands.iter().any(|command| {
            matches!(
                command,
                AnimationCommand::SetFrame(_)
                    | AnimationCommand::SetPlaySettings(_)
                    | AnimationCommand::Rewind
            )
        }) && frame != player.frame() as i32
        {
            player.goto(frame);
        }
        for command in commands {
            match command {
                AnimationCommand::SetFrame(frame) if frame != player.frame() as i32 => {
                    player.goto(frame)
                }
                AnimationCommand::SetFrame(_) => {}
                AnimationCommand::Advance(seconds) => {
                    player.advance_time(seconds);
                }
                AnimationCommand::SetPlaySettings(settings) => {
                    player.set_play_settings(settings);
                }
                AnimationCommand::Rewind => player.rewind(),
            }
        }
        if runtime.store().is_on_stage(object.0) {
            player.advance(dt);
        }
        if player.take_completion() {
            runtime.store_mut().report_animation_complete(object.0);
        }
        if let Some(mut image) = image {
            player.apply_frame(&mut image);
        }
        runtime
            .store_mut()
            .report_animation_frame(object.0, player.frame() as i32);
    }
}

/// Sources that resolved to a static image have no player to consume commands.
pub(crate) fn discard_unused_animation_commands(
    static_sources: Query<
        (&crate::GObjectRef, &crate::runtime::MovieSource),
        Without<MovieClipPlayer>,
    >,
    mut runtime: NonSendMut<crate::GuiRuntime>,
) {
    for (object, source) in &static_sources {
        let Some(gobject) = runtime.store().object(object.0) else {
            continue;
        };
        let fairygui_core::widget::ControlData::Loader(data) = gobject.control() else {
            continue;
        };
        let url = data.url.as_deref().unwrap_or("");
        if url.is_empty() || (source.url == url && source.visual.is_some()) {
            runtime.store_mut().take_animation_commands(object.0);
        }
    }
}

/// Blank frames remain part of the timeline but have no visible sprite.
pub fn sync_movie_visibility(
    mut query: Query<(&crate::GObjectRef, &MovieClipPlayer, &mut Visibility), With<ImageNode>>,
    runtime: NonSend<crate::GuiRuntime>,
) {
    for (object, player, mut visibility) in &mut query {
        let visible = runtime.store().is_on_stage(object.0)
            && runtime.store().display_visible(object.0)
            && player.current().is_some();
        let next = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != next {
            *visibility = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fairygui_core::geom::{Point, Rect, Size};

    #[test]
    fn catch_up_ignores_playing_and_speed_and_clears_fractional_time() {
        let mut player = MovieClipPlayer::new(&clip(4, 125, false, 0), 0, true).unwrap();
        player.advance(0.0625);
        player.stop();
        player.set_time_scale(0.0);
        player.advance_time(0.1875);
        assert_eq!(player.frame(), 1);
        player.play();
        player.set_time_scale(1.0);
        player.advance(0.0625);
        assert_eq!(player.frame(), 1);
        player.advance_time(0.0);
        player.advance(0.0625);
        assert_eq!(player.frame(), 1);
        player.advance(0.0625);
        assert_eq!(player.frame(), 2);
    }

    #[test]
    fn catch_up_handles_repeat_delay_frame_delays_and_large_swing_cycles() {
        for (swing, forward) in [(false, true), (true, false)] {
            let mut data = clip(4, 125, swing, 250);
            data.frames[2].add_delay = 125;
            let mut caught_up = MovieClipPlayer::new(&data, 0, true).unwrap();
            caught_up.advance_time(79.0 * 0.125);
            assert_eq!(
                (caught_up.frame, caught_up.forward, caught_up.repeated),
                (2, forward, true)
            );
        }
        let mut player = MovieClipPlayer::new(&clip(3, 125, false, 0), 0, false).unwrap();
        player.advance_time(1_000_000.0);
        assert_eq!(player.frame(), 2);
        let mut player = MovieClipPlayer::new(&clip(3, 100, false, 0), 0, false).unwrap();
        player.advance_time(1_000_000.0);
        assert_eq!(
            player.frame(),
            1,
            "large seeks retain exact millisecond cycles"
        );
        let mut player = MovieClipPlayer::new(&clip(4, 125, true, 0), 0, false).unwrap();
        player.advance_time(1_000_000.0);
        assert_eq!((player.frame(), player.forward), (2, true));
        let mut player = MovieClipPlayer::new(&clip(3, 0, true, 0), 0, false).unwrap();
        player.advance_time(1_000_000.0); // malformed zero-duration cycles terminate
        assert!(player.frame() < 3);
    }

    #[test]
    fn queued_commands_survive_missing_sources_and_seek_hidden_stopped_players_in_order() {
        use fairygui_core::package::ObjectType;
        let mut gui = crate::GuiRuntime::default();
        let loader = gui.store_mut().create(ObjectType::Loader);
        gui.store_mut().apply_icon_to(loader, "ui://pending");
        gui.store_mut().set_visible(loader, false);
        gui.store_mut().set_animation_playing(loader, false);
        gui.store_mut().advance_animation(loader, 0.375);
        gui.store_mut().set_animation_frame(loader, 1);
        gui.store_mut().advance_animation(loader, 0.125);
        let mut app = App::new();
        app.insert_non_send(gui)
            .init_resource::<Time>()
            .add_systems(
                Update,
                (advance_movie_clips, discard_unused_animation_commands).chain(),
            );
        let entity = app
            .world_mut()
            .spawn((
                crate::GObjectRef(loader),
                crate::runtime::MovieSource {
                    url: String::new(),
                    visual: None,
                },
            ))
            .id();
        app.update(); // unresolved marker must retain pending commands
        app.world_mut()
            .entity_mut(entity)
            .insert(MovieClipPlayer::new(&clip(4, 125, true, 0), 1, false).unwrap());
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(1));
        app.update();
        let player = app.world().get::<MovieClipPlayer>(entity).unwrap();
        assert_eq!(player.frame(), 2);
        assert!(!player.is_playing());
        assert_eq!(
            app.world()
                .non_send::<crate::GuiRuntime>()
                .store()
                .animation_state(loader),
            Some((false, 2))
        );
        app.update();
        assert_eq!(
            app.world().get::<MovieClipPlayer>(entity).unwrap().frame(),
            2
        );
        assert!(
            app.world_mut()
                .non_send_mut::<crate::GuiRuntime>()
                .store_mut()
                .take_animation_commands(loader)
                .is_empty()
        );
    }

    #[test]
    fn resolved_static_loader_discards_unused_movie_commands() {
        use fairygui_core::package::ObjectType;
        let mut gui = crate::GuiRuntime::default();
        let loader = gui.store_mut().create(ObjectType::Loader);
        gui.store_mut().apply_icon_to(loader, "picture.png");
        gui.store_mut().advance_animation(loader, 1.0);
        let mut app = App::new();
        app.insert_non_send(gui)
            .init_resource::<Time>()
            .add_systems(
                Update,
                (advance_movie_clips, discard_unused_animation_commands).chain(),
            );
        let visual = app.world_mut().spawn_empty().id();
        app.world_mut().spawn((
            crate::GObjectRef(loader),
            crate::runtime::MovieSource {
                url: "picture.png".into(),
                visual: Some(visual),
            },
        ));
        app.update();
        assert!(
            app.world_mut()
                .non_send_mut::<crate::GuiRuntime>()
                .store_mut()
                .take_animation_commands(loader)
                .is_empty()
        );
    }

    #[test]
    fn movie_speed_changes_preserve_fractional_frame_time() {
        let mut player = MovieClipPlayer::new(&clip(4, 125, false, 0), 0, true).unwrap();
        player.advance(0.0625);
        player.set_time_scale(2.0);
        player.advance(0.03125);
        assert_eq!(player.frame(), 1);
        player.set_time_scale(0.0);
        player.advance(1.0);
        assert_eq!(player.frame(), 1);
        player.set_time_scale(0.5);
        for _ in 0..4 {
            player.advance(0.0625);
        }
        assert_eq!(player.frame(), 2);
        assert!(!player.set_time_scale(f32::NAN));
        assert!(!player.set_time_scale(-1.0));
    }

    #[test]
    fn transition_movie_pause_and_speed_reach_the_bevy_player() {
        let package = fairygui_core::package::FuiPackage::parse(
            "Transition",
            include_bytes!("../../fairygui-core/tests/fixtures/Transition.fui"),
        )
        .unwrap();
        let mut gui = crate::GuiRuntime::default();
        let index = gui
            .create_root(Handle::default(), &package, "PowerUp")
            .unwrap();
        let root = gui.root_object(index).unwrap();
        gui.store_mut().attach_to_stage(root);
        let movie = gui.store().child_by_name(root, "powEffect").unwrap();
        let cue = gui
            .store()
            .object(root)
            .unwrap()
            .control()
            .component()
            .unwrap()
            .transitions[0]
            .items
            .iter()
            .find(|item| item.action == fairygui_core::transition::ActionType::Animation)
            .unwrap()
            .time;
        gui.store_mut().set_transition_time_scale(root, "t0", 2.0);
        gui.store_mut().play_transition(root, "t0", 1);
        gui.store_mut().advance_transitions(cue / 2.0 + 0.01);
        assert_eq!(gui.store().animation_time_scale(movie), Some(2.0));
        let mut app = App::new();
        app.insert_non_send(gui)
            .init_resource::<Time>()
            .add_systems(Update, advance_movie_clips);
        let entity = app
            .world_mut()
            .spawn((
                crate::GObjectRef(movie),
                MovieClipPlayer::new(&clip(4, 125, false, 0), 0, true).unwrap(),
            ))
            .id();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_micros(62500));
        app.update();
        assert_eq!(
            app.world().get::<MovieClipPlayer>(entity).unwrap().frame(),
            1
        );
        app.world_mut()
            .non_send_mut::<crate::GuiRuntime>()
            .store_mut()
            .set_transition_paused(root, "t0", true);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(500));
        app.update();
        assert_eq!(
            app.world().get::<MovieClipPlayer>(entity).unwrap().frame(),
            1
        );
        assert!(
            !app.world()
                .get::<MovieClipPlayer>(entity)
                .unwrap()
                .is_playing()
        );
        app.world_mut()
            .non_send_mut::<crate::GuiRuntime>()
            .store_mut()
            .set_transition_paused(root, "t0", false);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(125));
        app.update();
        let player = app.world().get::<MovieClipPlayer>(entity).unwrap();
        assert_eq!(player.frame(), 2);
        assert_eq!(player.time_scale(), 2.0);
    }

    fn frame_at(x: f32) -> FrameTexture {
        FrameTexture {
            atlas_item: 0,
            rect: Rect::new(x, 0.0, 10.0, 10.0),
            offset: Point::ZERO,
            original_size: Size::new(10.0, 10.0),
        }
    }

    fn clip(frames: usize, interval_ms: i32, swing: bool, repeat_delay_ms: i32) -> MovieClipData {
        MovieClipData {
            interval: interval_ms,
            swing,
            repeat_delay: repeat_delay_ms,
            frames: (0..frames)
                .map(|index| fairygui_core::package::MovieFrame {
                    add_delay: 0,
                    sprite_id: None,
                    texture: Some(frame_at(index as f32 * 10.0)),
                })
                .collect(),
        }
    }

    #[test]
    fn interval_keeps_package_milliseconds() {
        let player = MovieClipPlayer::new(&clip(2, 100, false, 0), 0, true).expect("player");
        assert_eq!(player.frame_count(), 2);
        assert_eq!(player.interval, 100.0);
    }

    #[test]
    fn frames_advance_with_time() {
        let mut player = MovieClipPlayer::new(&clip(3, 100, false, 0), 0, true).expect("player");

        assert_eq!(player.frame(), 0);
        assert!(!player.advance(0.05), "不足一个间隔不应换帧");
        assert!(player.advance(0.06));
        assert_eq!(player.frame(), 1);
    }

    #[test]
    fn playback_loops_back_to_the_first_frame() {
        let mut player = MovieClipPlayer::new(&clip(2, 100, false, 0), 0, true).expect("player");

        player.advance(0.11);
        assert_eq!(player.frame(), 1);
        player.advance(0.11);
        assert_eq!(player.frame(), 0, "播放到最后应当回到第一帧");
    }

    #[test]
    fn repeat_delay_pauses_before_looping() {
        let mut player = MovieClipPlayer::new(&clip(2, 100, false, 200), 0, true).expect("player");

        // 播放到末帧。
        player.advance(0.11);
        assert_eq!(player.frame(), 1);

        // 触发循环，进入等待。
        player.advance(0.11);
        assert_eq!(player.frame(), 0);

        // 等待期内不应前进。
        player.advance(0.15);
        assert_eq!(player.frame(), 0, "循环延迟内应当停住");
    }

    #[test]
    fn ordinary_swing_updates_respect_default_range_restart_at_the_end() {
        let mut player = MovieClipPlayer::new(&clip(3, 100, true, 0), 0, true).expect("player");

        player.advance(0.11);
        assert_eq!(player.frame(), 1);
        player.advance(0.11);
        assert_eq!(player.frame(), 2);

        // update's pending loop restart overrides the reverse step, as in Laya.
        player.advance(0.11);
        assert_eq!(player.frame(), 0);
        player.advance(0.11);
        assert_eq!(player.frame(), 0);

        // 回到首帧后再正放。
        player.advance(0.11);
        assert_eq!(player.frame(), 1);
    }

    #[test]
    fn stopped_player_does_not_advance() {
        let mut player = MovieClipPlayer::new(&clip(3, 100, false, 0), 0, false).expect("player");
        assert!(!player.advance(1.0));
        assert_eq!(player.frame(), 0);
    }

    #[test]
    fn goto_sets_the_frame_and_resets_timing() {
        let mut player = MovieClipPlayer::new(&clip(5, 100, false, 0), 0, true).expect("player");

        player.goto(3);
        assert_eq!(player.frame(), 3);

        // 越界被夹住。
        player.goto(99);
        assert_eq!(player.frame(), 4);
        player.goto(-1);
        assert_eq!(player.frame(), 0);
    }

    #[test]
    fn empty_clip_yields_no_player() {
        let empty = MovieClipData {
            interval: 100,
            swing: false,
            repeat_delay: 0,
            frames: Vec::new(),
        };
        assert!(MovieClipPlayer::new(&empty, 0, true).is_none());
    }

    #[test]
    fn blank_frames_keep_their_timeline_indices_and_per_frame_delay() {
        let mut clip = clip(3, 100, false, 0);
        clip.frames[1].texture = None;
        clip.frames[1].add_delay = 250;
        let mut player = MovieClipPlayer::new(&clip, 0, true).unwrap();
        assert_eq!(player.frame_count(), 3);
        player.advance(0.125);
        assert_eq!(player.frame(), 1);
        assert!(player.current().is_none());
        for _ in 0..3 {
            player.advance(0.1);
        }
        assert_eq!(player.frame(), 1);
        player.advance(0.125);
        assert_eq!(player.frame(), 2);
        assert_eq!(player.current().unwrap().rect.x, 20.0);
    }

    #[test]
    fn frames_switch_atlases_and_loop_restart_discards_leftover_time() {
        let mut clip = clip(2, 100, false, 250);
        clip.frames[1].texture.as_mut().unwrap().atlas_item = 1;
        let mut player = MovieClipPlayer::new(&clip, 0, true).unwrap();
        let mut assets = Assets::<Image>::default();
        let first = assets.add(Image::default());
        let second = assets.add(Image::default());
        player.bind_atlas(0, first.clone());
        player.bind_atlas(1, second.clone());
        let mut image = ImageNode::default();
        player.apply_frame(&mut image);
        assert_eq!(image.image, first);
        player.advance(0.125);
        player.apply_frame(&mut image);
        assert_eq!(image.image, second);
        player.set_time_scale(4.0);
        player.advance(0.25); // Restart discards the capped fractional remainder.
        assert_eq!(player.frame(), 0);
        player.set_time_scale(1.0);
        for _ in 0..3 {
            player.advance(0.1);
        }
        assert_eq!(player.frame(), 0);
        player.advance(0.05);
        assert_eq!(player.frame(), 1);
    }

    #[test]
    fn model_commands_seek_stopped_players_and_live_frames_are_reported_back() {
        use fairygui_core::package::ObjectType;
        let mut gui = crate::GuiRuntime::default();
        let movie = gui.store_mut().create(ObjectType::MovieClip);
        gui.store_mut().attach_to_stage(movie);
        gui.store_mut().set_animation_playing(movie, true);
        let mut app = App::new();
        app.insert_non_send(gui)
            .init_resource::<Time>()
            .add_systems(Update, (advance_movie_clips, sync_movie_visibility).chain());
        let entity = app
            .world_mut()
            .spawn((
                crate::GObjectRef(movie),
                MovieClipPlayer::new(&clip(3, 100, false, 0), 0, true).unwrap(),
                ImageNode::default(),
                Visibility::Inherited,
            ))
            .id();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(125));
        app.update();
        assert_eq!(
            app.world()
                .non_send::<crate::GuiRuntime>()
                .store()
                .animation_state(movie),
            Some((true, 1))
        );
        {
            let mut gui = app.world_mut().non_send_mut::<crate::GuiRuntime>();
            gui.store_mut().set_animation_playing(movie, false);
            gui.store_mut().set_animation_frame(movie, 2);
        }
        app.update();
        let player = app.world().get::<MovieClipPlayer>(entity).unwrap();
        assert_eq!(player.frame(), 2);
        assert!(!player.is_playing());
        assert_eq!(
            app.world()
                .get::<ImageNode>(entity)
                .unwrap()
                .rect
                .unwrap()
                .min
                .x,
            20.0
        );
        app.update();
        assert_eq!(
            app.world().get::<MovieClipPlayer>(entity).unwrap().frame(),
            2
        );
    }

    #[test]
    fn blank_movie_frames_hide_the_sprite_without_stopping_playback() {
        use fairygui_core::package::ObjectType;
        let mut clip = clip(3, 100, false, 0);
        clip.frames[1].texture = None;
        let mut gui = crate::GuiRuntime::default();
        let movie = gui.store_mut().create(ObjectType::MovieClip);
        gui.store_mut().attach_to_stage(movie);
        gui.store_mut().set_animation_playing(movie, true);
        let mut app = App::new();
        app.insert_non_send(gui)
            .init_resource::<Time>()
            .add_systems(Update, (advance_movie_clips, sync_movie_visibility).chain());
        let entity = app
            .world_mut()
            .spawn((
                crate::GObjectRef(movie),
                MovieClipPlayer::new(&clip, 0, true).unwrap(),
                ImageNode::default(),
                Visibility::Inherited,
            ))
            .id();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(125));
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(entity).unwrap(),
            Visibility::Hidden
        );
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(entity).unwrap(),
            Visibility::Inherited
        );
        assert_eq!(
            app.world().get::<MovieClipPlayer>(entity).unwrap().frame(),
            2
        );
    }

    #[test]
    fn ordinary_large_delta_is_clamped_and_advances_only_one_frame() {
        let mut player = MovieClipPlayer::new(&clip(4, 100, false, 0), 0, true).expect("player");
        // Laya clamps the input before scaling, then takes at most one step.
        player.advance(0.35);
        assert_eq!(player.frame(), 1);
    }
}
