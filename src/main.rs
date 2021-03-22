use std::{
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

use jobserver::Client;

const THREADS: usize = 1000;

const TIMES: usize = 3;

fn workload() {
    let mut a = 0;

    (0..2600).for_each(|i| {
        let i = &i as *const i32;
        a += unsafe { i.read_volatile() };
    });
    
    let mut b = 0;
    let b = &mut b as *mut i32;
    unsafe { b.write_volatile(a) };
}

fn main() {
    let mut times = Vec::new();

    for _ in 0..TIMES {
        let starter = Arc::new((parking_lot::Mutex::new(()), parking_lot::Condvar::new()));

        let done = Arc::new((Mutex::new(false), Condvar::new()));

        let sync = Arc::new((Mutex::new(0), Condvar::new()));

        let threads: Vec<_> = (0..THREADS)
            .map(|_| {
                let starter = starter.clone();
                let done = done.clone();
                let sync = sync.clone();
                std::thread::spawn(move || {
                    {
                        let mut lock = starter.0.lock();
                        starter.1.wait(&mut lock);
                    }

                    workload();
                    
                    {
                        let mut lock = sync.0.lock().unwrap();
                        *lock += 1;
                        if *lock == THREADS {
                            sync.1.notify_one();
                        }
                    }

                    starter.1.notify_one();

                    // Wait until benchmark is done
                    {
                        let mut lock = done.0.lock().unwrap();

                        while !*lock {
                            lock = done.1.wait(lock).unwrap();
                        }
                    }
                })
            })
            .collect();

        std::thread::sleep(Duration::from_millis(2000));

        let start = Instant::now();

        {
            let mut lock = sync.0.lock().unwrap();

            starter.1.notify_one();

            while *lock != THREADS {
                lock = sync.1.wait(lock).unwrap();
            }
        }

        let duration = start.elapsed();

        *done.0.lock().unwrap() = true;
        done.1.notify_all();

        for thread in threads {
            thread.join().unwrap();
        }

        times.push(duration.as_secs_f64());
    }

    let time = times.iter().copied().sum::<f64>() / (times.len() as f64);

    println!("control: {:.02}", time * 1000.0);


    let mut times = Vec::new();

    for _ in 0..TIMES {
        let client = Arc::new(Client::new(1).unwrap());

        client.acquire_raw().unwrap();

        let done = Arc::new((Mutex::new(false), Condvar::new()));

        let sync = Arc::new((Mutex::new(0), Condvar::new()));

        let threads: Vec<_> = (0..THREADS)
            .map(|_| {
                let client = client.clone();
                let done = done.clone();
                let sync = sync.clone();
                std::thread::spawn(move || {
                    client.acquire_raw().unwrap();

                    workload();

                    {
                        let mut lock = sync.0.lock().unwrap();
                        *lock += 1;
                        if *lock == THREADS {
                            sync.1.notify_one();
                        }
                    }

                    client.release_raw().unwrap();
       
                    // Wait until benchmark is done
                    {
                        let mut lock = done.0.lock().unwrap();

                        while !*lock {
                            lock = done.1.wait(lock).unwrap();
                        }
                    }
                })
            })
            .collect();

        std::thread::sleep(Duration::from_millis(2000));

        let start = Instant::now();

        {
            let mut lock = sync.0.lock().unwrap();

            client.release_raw().unwrap();

            while *lock != THREADS {
                lock = sync.1.wait(lock).unwrap();
            }
        }

        let duration = start.elapsed();

        *done.0.lock().unwrap() = true;
        done.1.notify_all();

        for thread in threads {
            thread.join().unwrap();
        }

        times.push(duration.as_secs_f64());
    }

    let time = times.iter().copied().sum::<f64>() / (times.len() as f64);

    println!("jobserver: {:.02}", time * 1000.0);
}
