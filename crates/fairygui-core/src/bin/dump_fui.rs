//! 打印 `.fui` 包的结构，用于脱离渲染层核对解析结果。
//!
//! 用法：
//!
//! ```text
//! cargo run -p fairygui-core --bin dump_fui -- <package.fui> [--items] [--hex]
//! ```

use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use fairygui_core::object::ObjectStore;
use fairygui_core::package::{FuiPackage, LoadContext, PackageItemType};
use fairygui_core::widget::create_from_package;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let Some(path) = args.first() else {
        eprintln!("usage: dump_fui <package.fui> [--items] [--hex]");
        return ExitCode::from(2);
    };
    let verbose = args.iter().any(|arg| arg == "--items");
    let hex = args.iter().any(|arg| arg == "--hex");
    let tree = args.iter().any(|arg| arg == "--tree");

    let data = match fs::read(path) {
        Ok(data) => data,
        Err(err) => {
            eprintln!("failed to read {path}: {err}");
            return ExitCode::FAILURE;
        }
    };

    // 资源键决定图集与声音的文件名前缀，这里用文件名（不含扩展名）近似。
    let res_key = Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("package");

    let package = match FuiPackage::parse(res_key, &data) {
        Ok(package) => package,
        Err(err) => {
            eprintln!("failed to parse {path}: {err}");
            return ExitCode::FAILURE;
        }
    };

    println!("package   : {} ({})", package.name(), package.id());
    println!("res key   : {}", package.res_key());
    println!("version   : {}", package.version());
    println!("data size : {} bytes", package.data().len());
    println!("items     : {}", package.item_count());

    if !package.dependencies().is_empty() {
        println!("dependencies:");
        for dependency in package.dependencies() {
            println!("  {} ({})", dependency.name, dependency.id);
        }
    }
    if !package.branches().is_empty() {
        println!("branches  : {}", package.branches().join(", "));
    }

    if verbose {
        dump_items(&package, hex);
    }
    if tree {
        dump_trees(&package);
    }

    ExitCode::SUCCESS
}

fn dump_trees(package: &FuiPackage) {
    let context = LoadContext::none();
    let mut store = ObjectStore::new();
    for (index, item) in package.items().iter().enumerate() {
        if item.item_type != PackageItemType::Component {
            continue;
        }
        let Ok(root) = create_from_package(&mut store, package, index, &context) else {
            continue;
        };
        println!("\n{}:", item.name);
        dump_object_tree(&store, root, 0);
        store.remove(root);
    }
}

fn dump_object_tree(store: &ObjectStore, id: fairygui_core::object::GObjectId, depth: usize) {
    let Some(object) = store.object(id) else {
        return;
    };
    println!(
        "{}{} ({:?})",
        "  ".repeat(depth),
        object.name(),
        object.kind()
    );
    for child in object.children() {
        dump_object_tree(store, *child, depth + 1);
    }
}

fn dump_items(package: &FuiPackage, hex: bool) {
    let ctx = LoadContext::none();

    println!();
    println!("items:");

    for index in 0..package.item_count() {
        let Some(item) = package.item(index) else {
            continue;
        };

        let mut notes = String::new();
        if let Some(sprite) = package.sprite_for(index) {
            notes.push_str(&format!(
                " sprite={},{} {}x{}",
                sprite.rect.x, sprite.rect.y, sprite.rect.width, sprite.rect.height
            ));
            if sprite.rotated {
                notes.push_str(" rotated");
            }
        }
        if item.scale9_grid.is_some() {
            notes.push_str(" scale9");
        }
        if item.scale_by_tile {
            notes.push_str(" tile");
        }
        if let Some(raw) = &item.raw_data {
            notes.push_str(&format!(" raw={}B", raw.len()));
        }
        if item.pixel_hit_test_data.is_some() {
            notes.push_str(" pixelHitTest");
        }
        if let Some(branches) = &item.branches {
            notes.push_str(&format!(" branches={}", branches.len()));
        }
        if let Some(hd) = &item.high_resolution {
            notes.push_str(&format!(" highRes={}", hd.len()));
        }

        println!(
            "  [{index:3}] {:<11} {:<24} {:>5}x{:<5} file={:<28}{}",
            item.item_type.as_str(),
            item.name,
            item.width,
            item.height,
            item.file.as_deref().unwrap_or("-"),
            notes
        );

        if hex && let Some(range) = &item.raw_data {
            let bytes = &package.data()[range.clone()];
            let preview = &bytes[..bytes.len().min(48)];
            let text = preview
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<Vec<_>>()
                .join(" ");
            println!("        raw: {text}");
        }

        match item.item_type {
            PackageItemType::Font => match package.load_bitmap_font(index, &ctx) {
                Ok(font) => println!(
                    "        -> bitmap font: ttf={} tint={} autoScale={} size={} xadvance={} lineHeight={} glyphs={}",
                    font.ttf,
                    font.tint,
                    font.auto_scale_size,
                    font.font_size,
                    font.x_advance,
                    font.line_height,
                    font.glyphs.len()
                ),
                Err(err) => println!("        -> bitmap font failed: {err}"),
            },
            PackageItemType::MovieClip => match package.load_movie_clip(index, &ctx) {
                Ok(clip) => println!(
                    "        -> movie clip: {} frames, interval {}, swing {}, repeatDelay {}",
                    clip.frames.len(),
                    clip.interval,
                    clip.swing,
                    clip.repeat_delay
                ),
                Err(err) => println!("        -> movie clip failed: {err}"),
            },
            _ => {}
        }
    }
}
