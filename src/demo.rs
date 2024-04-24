fn main() -> anyhow::Result<()> {
    pollster::block_on(tracer::run())
}