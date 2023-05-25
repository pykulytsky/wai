use criterion::*;
use std::sync::Mutex;
use std::thread;
use wal::LinkedList;

fn pop_front(c: &mut Criterion) {
    let mut group = c.benchmark_group("pop_front");
    for t in 1..=4 {
        group.throughput(criterion::Throughput::Elements(t as u64));

        group.bench_with_input(BenchmarkId::new("std::linked_list", t), &t, |b, &t| {
            let list = Mutex::new(std::collections::LinkedList::from_iter(0..10000));
            b.iter(|| {
                thread::scope(|s| {
                    for _ in 1..=t {
                        s.spawn(|| {
                            for _ in 0..1000 {
                                let _ = list.lock().unwrap().pop_front();
                            }
                        });
                    }
                });
            });
        });

        group.bench_with_input(BenchmarkId::new("wal", t), &t, |b, &t| {
            let list = LinkedList::new();
            for i in 0..10000 {
                list.push_back(i);
            }
            b.iter(|| {
                thread::scope(|s| {
                    for _ in 1..=t {
                        s.spawn(|| {
                            for _ in 0..1000 {
                                let _ = list.pop_front();
                            }
                        });
                    }
                });
            });
        });
    }
}

criterion_group!(benches, pop_front);
criterion_main!(benches);
