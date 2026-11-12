use crate::{object::ObjectStore, package::ObjectType};

#[test]
fn native_stroke_setter_changes_and_text_versus_bitmap_draw_commands_match() {
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::Text);
    let mut changes = 0;
    let mut count = [0; 4];
    for line in include_str!("../../tests/fixtures/text-stroke-reference.txt").lines() {
        if line.starts_with('#') {
            continue;
        }
        let p: Vec<_> = line.split_whitespace().collect();
        match p[0] {
            "width" => {
                let width: f32 = p[1].parse().unwrap();
                changes += usize::from(store.set_object_stroke(owner, width));
                assert_eq!(
                    store
                        .object(owner)
                        .unwrap()
                        .control()
                        .as_text_field()
                        .unwrap()
                        .stroke,
                    p[2].parse::<f32>().unwrap()
                );
                assert_eq!(changes, p[3].parse::<usize>().unwrap(), "{line}");
                count[0] += 1;
            }
            "color" => {
                assert_eq!(p[1], p[2]);
                count[1] += 1;
            }
            "command" => {
                if p[1] == "1" {
                    assert_eq!(p[3], "image");
                } else {
                    assert_eq!(p[3], "text");
                    assert_eq!(p[2], p[4]);
                    assert_eq!(p[5], "#cc1122");
                }
                count[2] += 1;
            }
            "raster" => {
                let width: f32 = p[1].parse().unwrap();
                assert_eq!(p[3].parse::<f32>().unwrap(), width.max(0.));
                assert_eq!(p[4], if width > 0. { "round" } else { "-" });
                assert_eq!(p[5], if width > 0. { "stroke,fill" } else { "fill" });
                count[3] += 1;
            }
            _ => panic!("{line}"),
        }
    }
    assert_eq!(count, [7, 4, 6, 10]);
}
