//! Parser benchmarks.

use criterion::{criterion_group, criterion_main, Criterion, black_box};
use seed_parser::parse_document;

const SIMPLE_DOC: &str = r#"
Frame Button:
  fill: #3B82F6
  constraints:
    - width = 120px
    - height = 40px
"#;

const MEDIUM_DOC: &str = include_str!("../tests/fixtures/card.seed");

fn parse_simple(c: &mut Criterion) {
    c.bench_function("parse_simple", |b| {
        b.iter(|| parse_document(black_box(SIMPLE_DOC)))
    });
}

fn parse_medium(c: &mut Criterion) {
    c.bench_function("parse_medium", |b| {
        b.iter(|| parse_document(black_box(MEDIUM_DOC)))
    });
}

criterion_group!(benches, parse_simple, parse_medium);
criterion_main!(benches);
