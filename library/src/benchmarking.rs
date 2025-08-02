use std::collections::HashMap;
use std::time::{Duration, Instant};

pub enum Benchmark {
    Disabled,
    JustTotalDuration(Duration),
    Enabled {
        sub_start_time: Option<Instant>,
        total_duration: Duration,
        tasks: HashMap<String, (usize, Benchmark)>,
    },
}

#[macro_export]
macro_rules! benchmark {
    ($bench:ident,$name:expr,$s:stmt) => {
        $bench.start();
        $s
        $bench.complete($name)
    };
}

impl Benchmark {
    pub fn new() -> Self {
        Benchmark::Enabled {
            sub_start_time: None,
            total_duration: Duration::new(0, 0),
            tasks: HashMap::new(),
        }
    }

    pub(crate) fn empty_with_total_duration(duration: Duration) -> Self {
        Benchmark::JustTotalDuration(duration)
    }

    pub fn is_enabled(&self) -> bool {
        matches!(self, Benchmark::Enabled { .. })
    }

    fn print_times_internal(&self, indent: usize, name: &str) {
        let indent_str = "|  ".repeat(indent);
        match self {
            Benchmark::Disabled => {
                println!("{indent_str}{name}: Benchmarking is disabled");
                return;
            },
            Benchmark::JustTotalDuration(duration) => {
                println!("{indent_str}{name}: {duration:?},");
                return;
            },
            Benchmark::Enabled { total_duration, tasks, .. } => {
                println!("{indent_str}{name}: {:?}", total_duration);
                let mut tasks_vec: Vec<_> = tasks.iter().collect();
                tasks_vec.sort_by_key(|(_, (idx, _))| *idx);
                for (name, (_idx, task)) in tasks_vec {
                    task.print_times_internal(indent + 1, name);
                }
            },
        }
    }

    pub fn print_times(&self) {
        self.print_times_internal(0, "Benchmark times");
        println!()
    }

    pub(crate) fn start(&mut self) {
        if let Benchmark::Enabled { sub_start_time, .. } = self {
            if sub_start_time.is_some() {
                panic!("Benchmark already started, cannot start again without completing the previous one.");
            }
            *sub_start_time = Some(Instant::now());
        }
    }

    pub(crate) fn complete(&mut self, name: &str) {
        if let Benchmark::Enabled { sub_start_time, total_duration, tasks } = self {
            let Some(duration) = sub_start_time.take().map(|t| t.elapsed()) else {
                panic!("Benchmark was not started, cannot complete.");
            };
            *total_duration += duration;
            tasks.insert(name.to_string(), (tasks.len(), Benchmark::empty_with_total_duration(duration)));
        }
    }

    pub(crate) fn add_task_with_benchmark(&mut self, name: &str, benchmark: Benchmark) {
        if let Benchmark::Enabled { tasks, total_duration, .. } = self {
            *total_duration += benchmark.get_total_duration();
            tasks.insert(name.to_string(), (tasks.len(), benchmark));
        }
    }

    pub(crate) fn get_total_duration(&self) -> Duration {
        match self {
            Benchmark::Disabled => Duration::ZERO,
            Benchmark::JustTotalDuration(duration) => *duration,
            Benchmark::Enabled { total_duration, .. } => *total_duration,
        }
    }

    pub(crate) fn new_sub_bench(&self) -> Self {
        match self {
            Benchmark::Disabled | Benchmark::JustTotalDuration(_) => Benchmark::Disabled,
            Benchmark::Enabled { .. } => Benchmark::new(),
        }
    }
}
