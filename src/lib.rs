use rclrs::{Node, WorkScope};
use std::{
    sync::{Arc, Condvar, Mutex, atomic::AtomicBool},
    thread::{self, JoinHandle},
    time::Duration,
};

pub use ros2_worker_node_derive::WorkerNode;

pub struct WorkerNode<Payload> {
    _node: Arc<Node>,
    _worker: rclrs::Worker<Payload>,
    _storage: Vec<Box<dyn std::any::Any>>,
}

impl<Payload> WorkerNode<Payload>
where
    Payload: WorkScope,
{
    pub fn new(node: &rclrs::Node, payload: Payload) -> Self {
        let worker = node.create_worker(payload);
        Self {
            _node: Arc::new(node.clone()),
            _worker: worker,
            _storage: Vec::new(),
        }
    }

    fn add_subscription<T>(&mut self, subscription: rclrs::WorkerSubscription<T, Payload>)
    where
        T: rosidl_runtime_rs::Message + 'static,
    {
        self._storage.push(Box::new(subscription));
    }

    fn add_service<S>(&mut self, service: rclrs::WorkerService<S, Payload>)
    where
        S: rosidl_runtime_rs::Service + 'static,
    {
        self._storage.push(Box::new(service));
    }

    // Helper methods to create and add in one go
    pub fn create_subscription<'a, T: rosidl_runtime_rs::Message, Args>(
        &mut self,
        options: impl Into<rclrs::SubscriptionOptions<'a>>,
        callback: impl rclrs::IntoWorkerSubscriptionCallback<T, Payload, Args>,
    ) -> Result<(), rclrs::RclrsError> {
        self.add_subscription(self._worker.create_subscription(options, callback)?);
        Ok(())
    }

    pub fn create_service<'a, T: rosidl_runtime_rs::Service, Args>(
        &mut self,
        options: impl Into<rclrs::ServiceOptions<'a>>,
        callback: impl rclrs::IntoWorkerServiceCallback<T, Payload, Args>,
    ) -> Result<(), rclrs::RclrsError> {
        self.add_service(self._worker.create_service::<T, Args>(options, callback)?);
        Ok(())
    }

    pub fn create_timer<Out, F>(&mut self, options: TimerOptions, callback: F) -> Arc<Timer>
    where
        F: Clone + FnOnce(&mut Payload) -> Out + 'static + Send,
        Out: 'static + Send,
        Payload: WorkScope,
    {
        let mut timer: Timer = Timer::new(options.clone());

        let worker_clone = self._worker.clone();

        let mut init_start = false;
        if options.auto_start {
            timer
                .running
                .store(true, std::sync::atomic::Ordering::SeqCst);
            timer
                .signal_pair
                .0
                .lock()
                .unwrap()
                .clone_from(&ExternTimerSignal::Run);
            init_start = true;
        }

        let signal_pair = timer.signal_pair.clone();
        let running_flag = timer.running.clone();
        let handle = thread::spawn(move || {
            let callback = callback;
            let running_flag = running_flag;
            let (lock, cvar) = &*signal_pair;

            loop {
                let mut signal = lock.lock().unwrap();
                while !init_start && *signal != ExternTimerSignal::Run {
                    running_flag.store(false, std::sync::atomic::Ordering::SeqCst);
                    if *signal == ExternTimerSignal::Dropped {
                        return;
                    }
                    signal = cvar.wait(signal).unwrap();
                }
                init_start = false;
                running_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                drop(signal);
                let mut _res = worker_clone.run(callback.clone());
                if options.single_shot {
                    *lock.lock().unwrap() = ExternTimerSignal::Stop;
                    continue;
                }
                thread::sleep(options.period);
            }
        });

        timer.init(handle);

        let timer_arc = Arc::new(timer);
        if options.globally_stored {
            self._storage.push(Box::new(timer_arc.clone()));
        }
        timer_arc
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExternTimerSignal {
    Run,
    Stop,
    Dropped,
}

#[derive(Debug, Clone)]
pub struct TimerOptions {
    pub period: Duration,
    pub single_shot: bool,
    pub auto_start: bool,
    globally_stored: bool,
}

impl TimerOptions {
    pub fn new(period: Duration) -> Self {
        Self {
            period,
            single_shot: false,
            auto_start: true,
            globally_stored: false,
        }
    }

    pub fn with_single_shot(self) -> Self {
        Self {
            single_shot: true,
            ..self
        }
    }

    pub fn without_auto_start(self) -> Self {
        Self {
            auto_start: false,
            ..self
        }
    }

    pub fn make_global(self) -> Self {
        Self {
            globally_stored: true,
            ..self
        }
    }
}

pub struct Timer {
    signal_pair: Arc<(Mutex<ExternTimerSignal>, Condvar)>,
    running: Arc<AtomicBool>,
    join_handle: Option<JoinHandle<()>>,
}

impl Timer {
    fn new(_options: TimerOptions) -> Self {
        Self {
            signal_pair: Arc::new((Mutex::new(ExternTimerSignal::Stop), Condvar::new())),
            running: Arc::new(AtomicBool::new(false)),
            join_handle: None,
        }
    }

    fn init(&mut self, join_handle: JoinHandle<()>) {
        self.join_handle = Some(join_handle);
    }

    pub fn start(&self) {
        let (lock, cvar) = &*self.signal_pair;
        let mut signal = lock.lock().unwrap();
        *signal = ExternTimerSignal::Run;
        cvar.notify_all(); // Wake the timer thread
    }

    pub fn stop(&self) {
        let (lock, cvar) = &*self.signal_pair;
        let mut signal = lock.lock().unwrap();
        self.running
            .store(false, std::sync::atomic::Ordering::SeqCst);
        *signal = ExternTimerSignal::Stop;
        cvar.notify_all(); // Wake the timer thread if waiting
    }

    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::SeqCst)
    }
}

//Logic: If the timer is dropped, the thread should also stop.
impl Drop for Timer {
    fn drop(&mut self) {
        if let Some(handle) = self.join_handle.take() {
            self.stop();
            // set to dropped
            let (lock, cvar) = &*self.signal_pair;
            let mut signal = lock.lock().unwrap();
            *signal = ExternTimerSignal::Dropped;
            cvar.notify_all();
            drop(signal);

            if let Err(err) = handle.join() {
                eprintln!("Timer thread panicked: {:?}", err);
            }
        }
    }
}
