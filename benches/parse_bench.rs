use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn go_source_snippet() -> &'static [u8] {
    b"package main\n\nimport (\n\t\"fmt\"\n\t\"os\"\n\t\"github.com/example/pkg/api\"\n\t\"github.com/example/pkg/db\"\n)\n\nfunc main() {\n\tfmt.Println(\"hello\")\n}\n"
}

fn python_source_snippet() -> &'static [u8] {
    b"import os\nimport sys\nfrom collections import OrderedDict\nfrom .utils import helper\nfrom ..config import settings\n\ndef main():\n    pass\n"
}

fn bench_go_parse(c: &mut Criterion) {
    let source = go_source_snippet();
    c.bench_function("go_tree_sitter_parse", |b| {
        b.iter(|| {
            let mut parser = tree_sitter::Parser::new();
            parser
                .set_language(&tree_sitter_go::LANGUAGE.into())
                .unwrap();
            let tree = parser.parse(black_box(source), None).unwrap();
            black_box(tree.root_node().child_count())
        })
    });
}

fn bench_python_parse(c: &mut Criterion) {
    let source = python_source_snippet();
    c.bench_function("python_tree_sitter_parse", |b| {
        b.iter(|| {
            let mut parser = tree_sitter::Parser::new();
            parser
                .set_language(&tree_sitter_python::LANGUAGE.into())
                .unwrap();
            let tree = parser.parse(black_box(source), None).unwrap();
            black_box(tree.root_node().child_count())
        })
    });
}

fn bench_go_parse_reuse_parser(c: &mut Criterion) {
    let source = go_source_snippet();
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_go::LANGUAGE.into())
        .unwrap();
    c.bench_function("go_tree_sitter_parse_reuse_parser", |b| {
        b.iter(|| {
            let tree = parser.parse(black_box(source), None).unwrap();
            black_box(tree.root_node().child_count())
        })
    });
}

criterion_group!(
    benches,
    bench_go_parse,
    bench_python_parse,
    bench_go_parse_reuse_parser
);
criterion_main!(benches);
