// mod old_benchmark {
// 	use std::time::{Duration, Instant};
//
// 	#[macro_export]
// 	macro_rules! benchmark {
// 		($bench:ident,$s:expr,$name:expr) => {{
// 			$bench.start();
// 			let return_val = $s;
// 			$bench.complete($name);
// 			return_val
// 		}};
// 	}
//
// 	#[derive(Clone)]
// 	pub enum Benchmark<'a> {
// 		Disabled,
// 		JustTotalDuration {
// 			name: String,
// 			total_duration: Duration,
// 		},
// 		Enabled {
// 			name: String,
// 			sub_start_time: Option<Instant>,
// 			total_duration: Duration,
// 			tasks: Vec<Benchmark<'a>>,
// 			parent: Option<&'a mut Benchmark<'a>>,
// 		},
// 	}
//
// 	impl Drop for Benchmark<'_> {
// 		fn drop(&mut self) {
// 			if let Benchmark::Enabled { name, sub_start_time, total_duration, tasks, parent } = self {
// 				if let Some(start_time) = sub_start_time.take() {
// 					let duration = start_time.elapsed();
// 					*total_duration += duration;
// 					tasks.push(Benchmark::empty_with_total_duration("Unfinished task".to_string(), duration));
// 				}
// 				if let Some(parent_bench) = parent {
// 					parent_bench.add_task_with_benchmark(self.clone());
// 				}
// 			}
// 		}
// 	}
//
// 	impl<'a> Benchmark<'a> {
// 		pub fn new(name: impl ToString) -> Self {
// 			Benchmark::Enabled {
// 				name: name.to_string(),
// 				sub_start_time: None,
// 				total_duration: Duration::new(0, 0),
// 				tasks: vec![],
// 				parent: None,
// 			}
// 		}
//
// 		fn empty_with_total_duration(name: String, duration: Duration) -> Self {
// 			Benchmark::JustTotalDuration { name, total_duration: duration }
// 		}
//
// 		pub fn is_enabled(&self) -> bool {
// 			matches!(self, Benchmark::Enabled { .. })
// 		}
//
// 		fn print_times_internal(&self, indent: usize, name: &str) {
// 			let indent_str = "|  ".repeat(indent);
// 			match self {
// 				Benchmark::Disabled => {
// 					println!("{indent_str}{name}: Benchmarking is disabled");
// 					return;
// 				},
// 				Benchmark::JustTotalDuration(duration) => {
// 					println!("{indent_str}{name}: {duration:?},");
// 					return;
// 				},
// 				Benchmark::Enabled { total_duration, tasks, .. } => {
// 					println!("{indent_str}{name}: {:?}", total_duration);
// 					for (name, task) in tasks {
// 						task.print_times_internal(indent + 1, name);
// 					}
// 				},
// 			}
// 		}
//
// 		pub fn print_times(&self) {
// 			self.print_times_internal(0, "Benchmark times");
// 			println!()
// 		}
//
// 		pub fn start(&mut self) {
// 			if let Benchmark::Enabled { sub_start_time, .. } = self {
// 				if sub_start_time.is_some() {
// 					panic!(
// 						"Benchmark already started, cannot start again without completing the previous one."
// 					);
// 				}
// 				*sub_start_time = Some(Instant::now());
// 			}
// 		}
//
// 		pub fn complete(&mut self, name: &str) {
// 			if let Benchmark::Enabled { sub_start_time, total_duration, tasks, .. } = self {
// 				let Some(duration) = sub_start_time.take().map(|t| t.elapsed()) else {
// 					panic!("Benchmark was not started, cannot complete.");
// 				};
// 				*total_duration += duration;
// 				tasks.push((name.to_string(), Benchmark::empty_with_total_duration(duration)));
// 			}
// 		}
//
// 		fn add_task_with_benchmark(&mut self, benchmark: Benchmark<'a>) {
// 			if let Benchmark::Enabled { tasks, total_duration, .. } = self {
// 				*total_duration += benchmark.get_total_duration();
// 				tasks.push(benchmark);
// 			}
// 		}
//
// 		pub(crate) fn get_total_duration(&self) -> Duration {
// 			match self {
// 				Benchmark::Disabled => Duration::ZERO,
// 				Benchmark::JustTotalDuration(duration) => *duration,
// 				Benchmark::Enabled { total_duration, .. } => *total_duration,
// 			}
// 		}
//
// 		pub(crate) fn new_sub_bench(&self) -> Self {
// 			match self {
// 				Benchmark::Disabled | Benchmark::JustTotalDuration(_) => Benchmark::Disabled,
// 				Benchmark::Enabled { .. } => Benchmark::new(),
// 			}
// 		}
//
// 		pub fn benchmark<T>(&mut self, name: &str, f: impl FnOnce() -> T) -> T {
// 			if !self.is_enabled() {
// 				return f();
// 			}
// 			self.start();
// 			let result = f();
// 			self.complete(name);
// 			result
// 		}
// 	}
// }

use std::mem;
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct Task {
	name: String,
	duration: Duration,
	sub_tasks: Option<Vec<Task>>,
}

pub struct TaskAdder {
	name: String,
	tasks: Vec<Task>,
}

impl TaskAdder {
	pub fn new(name: impl ToString) -> Self {
		TaskAdder { name: name.to_string(), tasks: vec![] }
	}

	pub fn benchmark<T>(&mut self, name: impl ToString, f: impl FnOnce() -> T) -> T {
		let t1 = Instant::now();
		let result = f();
		let duration = t1.elapsed();
		self.tasks.push(Task { name: name.to_string(), duration, sub_tasks: None });
		result
	}

	pub fn bench_with_inner<T>(&mut self, name: impl ToString, mut f: impl FnMut(&mut TaskAdder) -> T) -> T {
		let mut sub_adder = self.create_sub(name);
		let result = f(&mut sub_adder);
		self.tasks.push(sub_adder.finalize());
		result
	}

	fn create_sub(&mut self, name: impl ToString) -> TaskAdder {
		TaskAdder { name: name.to_string(), tasks: vec![] }
	}

	pub fn finalize(self) -> Task {
		let duration = self.tasks.iter().map(|t| t.duration).sum();
		Task { name: self.name.clone(), duration, sub_tasks: Some(self.tasks.iter().cloned().collect()) }
	}
}

impl Task {
	pub fn print(&self, indent: usize) {
		let indent_str = "|  ".repeat(indent);
		println!("{indent_str}{}: {:?}", self.name, self.duration);
		if let Some(sub_tasks) = &self.sub_tasks {
			for task in sub_tasks {
				task.print(indent + 1);
			}
		}
	}
}
