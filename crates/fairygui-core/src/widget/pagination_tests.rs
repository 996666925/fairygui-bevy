use super::*;

#[test]
fn materialized_pagination_positions_sizes_and_full_page_bounds_match_native() {
    let mut count = 0;
    for line in include_str!("../../tests/fixtures/pagination-reference.txt")
        .lines()
        .filter(|s| !s.starts_with('#'))
    {
        let v: Vec<f32> = line
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();
        let list = ListData {
            layout: ListLayoutType::Pagination,
            column_count: v[0] as i32,
            line_count: v[1] as i32,
            auto_resize_item: v[2] != 0.,
            column_gap: v[5] as i32,
            line_gap: v[6] as i32,
            ..Default::default()
        };
        let mut sizes = Vec::new();
        let mut sources = Vec::new();
        for i in 0..v[7] as usize {
            let (w, h, sw, sh) = match v[8] as i32 {
                0 => (100., 100., 100., 100.),
                1 => {
                    let w = if i % 2 != 0 { 35.5 } else { 28.25 };
                    let h = if i % 3 != 0 { 24.5 } else { 35.25 };
                    (w, h, w, h)
                }
                2 => (
                    if i % 2 != 0 { 44. } else { 30. },
                    if i % 3 != 0 { 19. } else { 30. },
                    if i % 2 != 0 { 22. } else { 20. },
                    if i % 3 != 0 { 19. } else { 30. },
                ),
                _ => {
                    let w = if i % 2 != 0 { 8. } else { 45. };
                    let h = if i % 3 != 0 { 36. } else { 12. };
                    (w, h, w, h)
                }
            };
            sizes.push(Size::new(w, h));
            sources.push(Size::new(sw, sh));
        }
        let result = list.pagination_layout(&sizes, &sources, Size::new(v[3], v[4]));
        let mut actual = vec![result.content.width, result.content.height];
        for (p, s) in result.positions.iter().zip(&result.sizes) {
            actual.extend([p.x, p.y, s.width, s.height]);
        }
        assert_eq!(actual.len(), v.len() - 9);
        for (a, e) in actual.iter().zip(&v[9..]) {
            assert!((a - e).abs() < 0.001, "actual={a} expected={e}: {line}");
        }
        count += 1;
    }
    assert_eq!(count, 577);
}
