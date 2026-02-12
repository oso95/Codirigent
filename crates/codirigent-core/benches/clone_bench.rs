use codirigent_core::{TaskId, SessionId};
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_task_id_clone(c: &mut Criterion) {
    let task_id = TaskId::from("task-001-with-a-longer-identifier");

    c.bench_function("TaskId::clone (Arc<str>)", |b| {
        b.iter(|| {
            let cloned = black_box(task_id.clone());
            cloned
        })
    });
}

fn bench_task_id_clone_many(c: &mut Criterion) {
    let task_ids: Vec<TaskId> = (0..100)
        .map(|i| TaskId::from(format!("task-{:03}", i)))
        .collect();

    c.bench_function("TaskId::clone 100x (Arc<str>)", |b| {
        b.iter(|| {
            let cloned: Vec<_> = task_ids.iter()
                .map(|id| black_box(id.clone()))
                .collect();
            cloned
        })
    });
}

fn bench_session_id_clone(c: &mut Criterion) {
    let session_id = SessionId(42);

    c.bench_function("SessionId::clone (u64)", |b| {
        b.iter(|| {
            let cloned = black_box(session_id.clone());
            cloned
        })
    });
}

criterion_group!(benches, bench_task_id_clone, bench_task_id_clone_many, bench_session_id_clone);
criterion_main!(benches);
