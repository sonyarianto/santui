use criterion::{criterion_group, criterion_main, Criterion};
use santui_ipc::protocol::{
    write_plugin_msg, HostMsg, IpcKey, IpcKeyModifiers, PluginMsg, RenderCmd, TextStyle,
};
use std::hint::black_box;

fn build_large_plugin_msg() -> PluginMsg {
    let mut rows = Vec::with_capacity(50);
    for i in 0..50 {
        rows.push(vec![
            format!("Row {i}"),
            format!("value_{i}_abcdef"),
            format!("{}", i * 1234),
        ]);
    }
    let commands = vec![
        RenderCmd::Clear {
            x: 0,
            y: 0,
            w: 120,
            h: 40,
        },
        RenderCmd::Text {
            x: 1,
            y: 1,
            text: "Header — My Plugin Title".into(),
            fg: Some([250, 178, 131]),
            bg: None,
            bold: true,
            modifiers: 0,
        },
        RenderCmd::Border {
            x: 0,
            y: 0,
            w: 120,
            h: 40,
            fg: [140; 3],
            borders: 15,
            bg: Some([20; 3]),
            title: Some("Window".into()),
            title_fg: Some([255; 3]),
            title_dash_fg: None,
            border_type: None,
        },
        RenderCmd::Table {
            x: 2,
            y: 3,
            w: 116,
            h: 35,
            header: vec!["Name".into(), "Value".into(), "Count".into()],
            header_style: TextStyle {
                fg: Some([255; 3]),
                bg: None,
                bold: true,
                modifiers: 0,
            },
            rows,
            column_widths: vec![30, 50, 20],
            selected: Some(12),
            style: TextStyle::default(),
            highlight_style: TextStyle {
                fg: None,
                bg: Some([40; 3]),
                bold: false,
                modifiers: 0,
            },
            current_row: Some(5),
            current_style: Some(TextStyle {
                fg: Some([0; 3]),
                bg: Some([255; 3]),
                bold: true,
                modifiers: 0,
            }),
            cell_styles: None,
        },
    ];
    PluginMsg {
        commands,
        hints: vec![
            ("↑↓".into(), "navigate".into()),
            ("Enter".into(), "select".into()),
            ("/".into(), "search".into()),
            ("Esc".into(), "back".into()),
        ],
        palette_commands: vec![],
        request: None,
        plugin_message: None,
        consumed: true,
    }
}

fn build_small_host_msg() -> HostMsg {
    HostMsg::Key {
        key: IpcKey::Char('j'),
        modifiers: IpcKeyModifiers {
            ctrl: false,
            alt: false,
            shift: false,
        },
    }
}

fn bench_json_plugin_msg_encode(c: &mut Criterion) {
    let msg = build_large_plugin_msg();
    c.bench_function("json/plugin_msg/encode", |b| {
        b.iter(|| {
            let json = serde_json::to_string(black_box(&msg)).unwrap();
            black_box(json)
        })
    });
}

fn bench_json_plugin_msg_decode(c: &mut Criterion) {
    let msg = build_large_plugin_msg();
    let json = serde_json::to_string(&msg).unwrap();
    c.bench_function("json/plugin_msg/decode", |b| {
        b.iter(|| {
            let decoded: PluginMsg = serde_json::from_str(black_box(&json)).unwrap();
            black_box(decoded)
        })
    });
}

fn bench_binary_plugin_msg_encode(c: &mut Criterion) {
    let msg = build_large_plugin_msg();
    c.bench_function("binary/plugin_msg/encode", |b| {
        let mut buf = Vec::with_capacity(4096);
        b.iter(|| {
            buf.clear();
            write_plugin_msg(black_box(&mut buf), black_box(&msg)).unwrap();
            black_box(buf.len())
        })
    });
}

fn bench_binary_plugin_msg_decode(c: &mut Criterion) {
    let msg = build_large_plugin_msg();
    let mut buf = Vec::with_capacity(4096);
    write_plugin_msg(&mut buf, &msg).unwrap();
    c.bench_function("binary/plugin_msg/decode", |b| {
        b.iter(|| {
            let mut cursor = std::io::Cursor::new(black_box(&buf));
            let decoded = santui_ipc::protocol::read_plugin_msg(black_box(&mut cursor)).unwrap();
            black_box(decoded)
        })
    });
}

fn bench_json_host_msg_encode(c: &mut Criterion) {
    let msg = build_small_host_msg();
    c.bench_function("json/host_msg/encode", |b| {
        b.iter(|| {
            let json = serde_json::to_string(black_box(&msg)).unwrap();
            black_box(json)
        })
    });
}

fn bench_json_host_msg_decode(c: &mut Criterion) {
    let msg = build_small_host_msg();
    let json = serde_json::to_string(&msg).unwrap();
    c.bench_function("json/host_msg/decode", |b| {
        b.iter(|| {
            let decoded: HostMsg = serde_json::from_str(black_box(&json)).unwrap();
            black_box(decoded)
        })
    });
}

criterion_group!(
    benches,
    bench_json_plugin_msg_encode,
    bench_json_plugin_msg_decode,
    bench_binary_plugin_msg_encode,
    bench_binary_plugin_msg_decode,
    bench_json_host_msg_encode,
    bench_json_host_msg_decode,
);
criterion_main!(benches);
