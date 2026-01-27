use crate::accumulator::AccumulatorConfig;
use crate::archiver::ArchiveConfig;
use crate::downstream::DownstreamConfig;
use crate::upstream::UpstreamConfig;

// TODO: implement this as clap config
pub struct Config {
    pub storage_path: String,

    pub upstream: UpstreamConfig,

    pub accumulator: AccumulatorConfig,

    pub archive: ArchiveConfig,

    pub downstream: DownstreamConfig,
    // TODO: rest goes here
}
