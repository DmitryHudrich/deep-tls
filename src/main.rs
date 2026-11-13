mod features;
mod knn;
mod net;
mod node;
mod rng;
mod tls;
mod wallet;

use knn::Knn;
use net::Defense;
use rng::Rng;
use wallet::Transaction;

const USAGE: &str = "usage: deep-tls [--seed N] [--per-class N] [--k N]";

struct Config {
    seed: u64,
    per_class: usize,
    k: usize,
}

fn parse_args() -> Result<Config, String> {
    let mut config = Config { seed: 42, per_class: 150, k: 5 };
    let args: Vec<String> = std::env::args().skip(1).collect();
    for pair in args.chunks(2) {
        let value: u64 = pair.get(1).and_then(|v| v.parse().ok()).ok_or(USAGE)?;
        match pair[0].as_str() {
            "--seed" => config.seed = value,
            "--per-class" => config.per_class = value.max(4) as usize,
            "--k" => config.k = value.max(1) as usize,
            _ => return Err(USAGE.into()),
        }
    }
    Ok(config)
}

type Sample = (Vec<f64>, Transaction);

fn observe(tx: Transaction, defense: Defense, rng: &mut Rng) -> Vec<tls::Record> {
    let exchanges = wallet::run(tx, rng);
    records_of(&exchanges, defense, rng)
}

fn records_of(exchanges: &[wallet::Exchange], defense: Defense, rng: &mut Rng) -> Vec<tls::Record> {
    tls::parse_records(&net::capture(exchanges, defense, rng)).expect("capture is valid TLS")
}

fn dataset(per_class: usize, defense: Defense, rng: &mut Rng) -> Vec<Sample> {
    let mut data: Vec<Sample> = Transaction::ALL
        .iter()
        .flat_map(|&tx| std::iter::repeat_n(tx, per_class))
        .map(|tx| (features::extract(&observe(tx, defense, rng)), tx))
        .collect();
    rng.shuffle(&mut data);
    data
}

fn evaluate(title: &str, defense: Defense, config: &Config, rng: &mut Rng) -> Knn<Transaction> {
    let data = dataset(config.per_class, defense, rng);
    let (train, test) = data.split_at(data.len() * 7 / 10);
    let model = Knn::fit(train, config.k);

    let n = Transaction::ALL.len();
    let mut confusion = vec![vec![0usize; n]; n];
    for (x, actual) in test {
        confusion[*actual as usize][model.predict(x) as usize] += 1;
    }
    let correct: usize = (0..n).map(|i| confusion[i][i]).sum();

    println!("\n== {title}: точность {:.1}% ({correct}/{})", 100.0 * correct as f64 / test.len() as f64, test.len());
    print!("{:>12}", "факт \\ прогн");
    Transaction::ALL.iter().for_each(|tx| print!("{:>11}", tx.name()));
    println!();
    for (tx, row) in Transaction::ALL.iter().zip(&confusion) {
        print!("{:>12}", tx.name());
        row.iter().for_each(|count| print!("{count:>11}"));
        println!();
    }
    model
}

fn trace_preview(records: &[tls::Record]) -> String {
    let arrows = records.iter().take(10).map(|r| match r.dir {
        tls::Direction::Up => format!("↑{}", r.len),
        tls::Direction::Down => format!("↓{}", r.len),
    });
    let tail = if records.len() > 10 { format!(" … (+{})", records.len() - 10) } else { String::new() };
    format!("{}{tail}", arrows.collect::<Vec<_>>().join(" "))
}

fn main() {
    let config = parse_args().unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2);
    });
    let mut rng = Rng::new(config.seed);

    println!("Сессии кошелька с тестовым узлом (Ethereum JSON-RPC / Tron HTTP API) поверх TLS 1.3.");
    println!("Наблюдатель видит только длины и тайминги TLS-записей. {} сессий на класс, k={}.", config.per_class, config.k);

    let model = evaluate("Без защиты", Defense::None, &config, &mut rng);

    println!("\nНовые сессии, которых не было в обучении:");
    for tx in Transaction::ALL {
        let exchanges = wallet::run(tx, &mut rng);
        let records = records_of(&exchanges, Defense::None, &mut rng);
        let predicted = model.predict(&features::extract(&records));
        let mark = if predicted == tx { "✓" } else { "✗" };
        let calls: Vec<&str> = exchanges.iter().map(|e| e.call.trim_start_matches("/wallet/")).collect();
        println!("  {mark} {:<10} → {:<10} {}", tx.name(), predicted.name(), trace_preview(&records));
        println!("      вызовы: {}", calls.join(", "));
    }

    evaluate("Паддинг записей до 1 KiB", Defense::Padding(1024), &config, &mut rng);
    evaluate("Паддинг записей до 16 KiB", Defense::Padding(16384), &config, &mut rng);
}
