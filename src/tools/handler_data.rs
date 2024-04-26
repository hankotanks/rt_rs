use rt::handlers;
use rt::handlers::IntrsHandler;

fn main() -> anyhow::Result<()> {
    let config = handlers::BvhConfig {
        eps: 0.000002,
    };

    handlers::BvhIntrs::set_data(config);

    Ok(())
}