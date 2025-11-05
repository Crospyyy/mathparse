use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct Task {
	name: String,
	duration: Duration,
	sub_tasks: Option<Vec<Task>>,
}

pub struct Benchmark {
	name: String,
	tasks: Vec<Task>,
}

impl Benchmark {
	pub fn new(name: impl ToString) -> Self {
		Benchmark { name: name.to_string(), tasks: vec![] }
	}

	pub fn benchmark<T>(&mut self, name: impl ToString, f: impl FnOnce() -> T) -> T {
		let t1 = Instant::now();
		let result = f();
		let duration = t1.elapsed();
		self.tasks.push(Task { name: name.to_string(), duration, sub_tasks: None });
		result
	}

	pub fn bench_with_inner<T>(&mut self, name: impl ToString, mut f: impl FnMut(&mut Benchmark) -> T) -> T {
		let mut sub_adder = self.create_sub(name);
		let result = f(&mut sub_adder);
		self.tasks.push(sub_adder.finalize());
		result
	}

	fn create_sub(&mut self, name: impl ToString) -> Benchmark {
		Benchmark { name: name.to_string(), tasks: vec![] }
	}

	pub fn finalize(self) -> Task {
		let duration = self.tasks.iter().map(|t| t.duration).sum();
		Task { name: self.name.clone(), duration, sub_tasks: Some(self.tasks.clone()) }
	}
}

impl Task {
	fn print_internal(&self, indent: usize) {
		let indent_str = "|  ".repeat(indent);
		println!("{indent_str}{}: {:?}", self.name, self.duration);
		if let Some(sub_tasks) = &self.sub_tasks {
			for task in sub_tasks {
				task.print_internal(indent + 1);
			}
		}
	}
	pub fn print(&self) {
        println!();
		self.print_internal(0);
	}
}
