extern crate reqwest;

mod sensorcloud;

fn main() {
    let mut dev = sensorcloud::Device::new("device_id", "device_key");
    
    let first_time: u64 = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() * 1000000000;

    let data = vec![
        sensorcloud::Point { timestamp: first_time, value: 1.0, },
        sensorcloud::Point { timestamp: first_time + 1000000000, value: 2.0, },
        sensorcloud::Point { timestamp: first_time + 2000000000, value: 3.5, },
    ];
    
    let sample_rate = sensorcloud::SampleRate::hertz(1);
    dev.upload_data("rust", "ch1", &sample_rate, &data).unwrap();
}
