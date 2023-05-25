use criterion::*;
use std::sync::Mutex;
use std::thread;
use wal::doubly;
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

        group.bench_with_input(BenchmarkId::new("wal::doubly", t), &t, |b, &t| {
            let list = doubly::LinkedList::new();
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

fn pop_back(c: &mut Criterion) {
    let mut group = c.benchmark_group("pop_back");
    for t in 1..=4 {
        group.throughput(criterion::Throughput::Elements(t as u64));

        group.bench_with_input(BenchmarkId::new("std::linked_list", t), &t, |b, &t| {
            let list = Mutex::new(std::collections::LinkedList::from_iter(0..10000));
            b.iter(|| {
                thread::scope(|s| {
                    for _ in 1..=t {
                        s.spawn(|| {
                            for _ in 0..1000 {
                                let _ = list.lock().unwrap().pop_back();
                            }
                        });
                    }
                });
            });
        });

        group.bench_with_input(BenchmarkId::new("wal::doubly", t), &t, |b, &t| {
            let list = doubly::LinkedList::new();
            for i in 0..10000 {
                list.push_back(i);
            }
            b.iter(|| {
                thread::scope(|s| {
                    for _ in 1..=t {
                        s.spawn(|| {
                            for _ in 0..1000 {
                                let _ = list.pop_back();
                            }
                        });
                    }
                });
            });
        });
    }
}

fn push_front(c: &mut Criterion) {
    let mut group = c.benchmark_group("push_front");
    for t in 1..=4 {
        group.throughput(criterion::Throughput::Elements(t as u64));

        group.bench_with_input(BenchmarkId::new("std::linked_list", t), &t, |b, &t| {
            let list = Mutex::new(std::collections::LinkedList::new());
            b.iter(|| {
                thread::scope(|s| {
                    for _ in 1..=t {
                        s.spawn(|| {
                            for i in 0..100 {
                                let _ = list.lock().unwrap().push_front(i);
                            }
                        });
                    }
                });
            });
        });

        group.bench_with_input(BenchmarkId::new("wal", t), &t, |b, &t| {
            let list = LinkedList::new();
            b.iter(|| {
                thread::scope(|s| {
                    for _ in 1..=t {
                        s.spawn(|| {
                            for i in 0..100 {
                                list.push_front(i);
                            }
                        });
                    }
                });
            });
        });

        group.bench_with_input(BenchmarkId::new("wal::doubly", t), &t, |b, &t| {
            let list = doubly::LinkedList::new();
            for i in 0..10000 {
                list.push_back(i);
            }
            b.iter(|| {
                thread::scope(|s| {
                    for _ in 1..=t {
                        s.spawn(|| {
                            for i in 0..100 {
                                let _ = list.push_front(i);
                            }
                        });
                    }
                });
            });
        });
    }
}

fn push_back(c: &mut Criterion) {
    let mut group = c.benchmark_group("push_back");
    for t in 1..=4 {
        group.throughput(criterion::Throughput::Elements(t as u64));

        group.bench_with_input(BenchmarkId::new("std::linked_list", t), &t, |b, &t| {
            let list = Mutex::new(std::collections::LinkedList::new());
            b.iter(|| {
                thread::scope(|s| {
                    for _ in 1..=t {
                        s.spawn(|| {
                            for i in 0..100 {
                                let _ = list.lock().unwrap().push_back(i);
                            }
                        });
                    }
                });
            });
        });

        group.bench_with_input(BenchmarkId::new("wal", t), &t, |b, &t| {
            let list = LinkedList::new();
            b.iter(|| {
                thread::scope(|s| {
                    for _ in 1..=t {
                        s.spawn(|| {
                            for i in 0..100 {
                                list.push_back(i);
                            }
                        });
                    }
                });
            });
        });

        group.bench_with_input(BenchmarkId::new("wal::doubly", t), &t, |b, &t| {
            let list = LinkedList::new();
            b.iter(|| {
                thread::scope(|s| {
                    for _ in 1..=t {
                        s.spawn(|| {
                            for i in 0..100 {
                                list.push_back(i);
                            }
                        });
                    }
                });
            });
        });
    }
}

criterion_group!(benches, pop_front, pop_back, push_front, push_back);
criterion_main!(benches);
