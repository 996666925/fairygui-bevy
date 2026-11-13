use super::*;
use crate::geom::Size;

#[test]
fn single_axis_layout_resizes_items_and_preserves_package_order() {
    let data = ListData {
        layout: ListLayoutType::SingleColumn,
        auto_resize_item: true,
        line_gap: 5,
        ..Default::default()
    };
    let original = [Size::new(40., 20.), Size::new(60., 30.)];
    let sources = original;
    let positions = [Point::new(3., 90.), Point::new(4., 120.)];
    let result =
        data.materialized_layout(&original, &sources, &positions, Size::new(100., 80.), true);
    assert_eq!(result.positions, [Point::new(3., 0.), Point::new(4., 25.)]);
    assert_eq!(result.sizes, [Size::new(100., 20.), Size::new(100., 30.)]);
    assert_eq!(result.content, Size::new(100., 55.));
}

#[test]
fn horizontal_auto_flow_fills_complete_rows_and_defers_partial_virtual_rows() {
    let data = ListData {
        layout: ListLayoutType::FlowHorizontal,
        auto_resize_item: true,
        column_count: 2,
        line_gap: 4,
        column_gap: 2,
        ..Default::default()
    };
    let sizes = [Size::new(20., 10.), Size::new(20., 12.), Size::new(20., 8.)];
    let positions = [Point::ZERO; 3];
    let partial = data.materialized_layout(&sizes, &sizes, &positions, Size::new(100., 80.), false);
    assert_eq!(
        partial.positions,
        [Point::new(0., 0.), Point::new(51., 0.), Point::ZERO]
    );
    assert_eq!(partial.content, Size::new(100., 16.));
    let full = data.materialized_layout(&sizes, &sizes, &positions, Size::new(100., 80.), true);
    assert_eq!(full.positions[2], Point::new(0., 16.));
    assert_eq!(full.sizes[2].width, 100.);
    assert_eq!(full.content.height, 28.);
}

#[test]
fn regular_flow_infers_rows_and_content_extent() {
    let data = ListData {
        layout: ListLayoutType::FlowHorizontal,
        column_gap: 5,
        line_gap: 7,
        ..Default::default()
    };
    let sizes = [
        Size::new(40., 20.),
        Size::new(40., 24.),
        Size::new(30., 10.),
    ];
    let positions = [Point::ZERO; 3];
    let result = data.materialized_layout(&sizes, &sizes, &positions, Size::new(85., 80.), true);
    assert_eq!(
        result.positions,
        [Point::new(0., 0.), Point::new(45., 0.), Point::new(0., 31.)]
    );
    assert_eq!(result.content, Size::new(85., 41.));
}
