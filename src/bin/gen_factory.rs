use reelsynth::wavetable::WavetableBank;

fn main() {
    let out_dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "data/wavetables".to_string());
    std::fs::create_dir_all(&out_dir).expect("create output dir");
    let tables = [
        ("saw_morph", WavetableBank::factory_saw_morph()),
        ("square_morph", WavetableBank::factory_square_morph()),
        ("sine", WavetableBank::factory_sine()),
        ("formant", WavetableBank::factory_formant()),
        ("metallic", WavetableBank::factory_metallic()),
        ("vocal_ah", WavetableBank::factory_formant()),
        ("bright_lead", WavetableBank::factory_saw_morph()),
        ("dark_pad", WavetableBank::factory_square_morph()),
    ];
    for (name, bank) in tables {
        let path = format!("{out_dir}/{name}.reelwt");
        bank.write_file(&path).expect("write reelwt");
        println!("{path}");
    }
}
