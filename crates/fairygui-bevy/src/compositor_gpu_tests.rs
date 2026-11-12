use super::*;

#[test]
fn premultiplied_blend_factors_match_upstream_shader_state_and_samples() {
    let input = include_str!("../../fairygui-core/tests/fixtures/blend-reference.txt");
    let mode = |s: &str| match s {
        "add" | "screen" => Mode::Add,
        "multiply" => Mode::Multiply,
        "mask" => Mode::Mask,
        "destinationOut" => Mode::Erase,
        _ => Mode::Normal,
    };
    let factor = |s: BlendFactor, src: &[f32], dst: &[f32], i: usize| match s {
        BlendFactor::Zero => 0.,
        BlendFactor::One => 1.,
        BlendFactor::SrcAlpha => src[3],
        BlendFactor::OneMinusSrcAlpha => 1. - src[3],
        BlendFactor::Dst => dst[i],
        BlendFactor::DstAlpha => dst[3],
        _ => panic!("unexpected factor {s:?}"),
    };
    let mut count = 0;
    for line in input.lines().filter(|l| l.starts_with("sample ")) {
        let p: Vec<_> = line.split_whitespace().collect();
        let src: Vec<f32> = p[2..6].iter().map(|v| v.parse().unwrap()).collect();
        let dst: Vec<f32> = p[6..10].iter().map(|v| v.parse().unwrap()).collect();
        let expected: Vec<f32> = p[10..14].iter().map(|v| v.parse().unwrap()).collect();
        let blend = blend(mode(p[1]), true);
        for i in 0..4 {
            let component = if i == 3 { blend.alpha } else { blend.color };
            let value = (src[i] * factor(component.src_factor, &src, &dst, i)
                + dst[i] * factor(component.dst_factor, &src, &dst, i))
            .min(1.);
            assert!(
                (value - expected[i]).abs() < 1e-6,
                "{line} channel {i}: {value}"
            );
        }
        count += 1;
    }
    assert_eq!(count, 36);
}

#[test]
fn batching_is_separated_at_group_and_multiply_boundaries() {
    let mut ops = Vec::new();
    let policy = PaintPolicy::default();
    append(&mut ops, &policy, Rect::new(0., 0., 10., 10.), 0..2, 0);
    append(&mut ops, &policy, Rect::new(10., 10., 20., 20.), 2..4, 0);
    assert!(matches!(&ops[..],[Op::Draw { range, multiply:false }] if *range == (0..4)));
    let multiply = PaintPolicy {
        blend: BlendMode::Multiply,
        ..default()
    };
    append(&mut ops, &multiply, Rect::default(), 4..5, 0);
    append(&mut ops, &multiply, Rect::default(), 5..6, 0);
    assert_eq!(ops.len(), 3);
}

#[test]
fn normal_mask_batches_are_replayed_after_content_regardless_of_child_order() {
    use fairygui_core::package::ObjectType;
    let mut gui = crate::GuiRuntime::default();
    let owner = gui.store_mut().create(ObjectType::Component);
    let mask = gui.store_mut().create(ObjectType::Graph);
    let content = gui.store_mut().create(ObjectType::Graph);
    gui.store_mut().set_parent(mask, Some(owner));
    gui.store_mut().set_parent(content, Some(owner));
    gui.store_mut().set_component_mask(owner, Some(mask), false);
    let mut ops = Vec::new();
    append(
        &mut ops,
        &super::super::policy(&gui, mask),
        Rect::default(),
        0..1,
        0,
    );
    append(
        &mut ops,
        &super::super::policy(&gui, content),
        Rect::default(),
        2..3,
        0,
    );
    let [
        Op::Group {
            group,
            children,
            mask_children,
            ..
        },
    ] = &ops[..]
    else {
        panic!("one isolated owner");
    };
    assert_eq!(group.mask, Some(mask));
    assert!(matches!(&children[..],[Op::Draw {range,..}] if *range==(2..3)));
    assert!(matches!(&mask_children[..],[Op::Group {group,..}] if group.id==mask));
}
