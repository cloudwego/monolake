# start monolake proxy service

if [ -z "${MONOLAKE_HOME+set}" ]; then
    export MONOLAKE_HOME=$HOME/monolake
fi

cd $MONOLAKE_HOME
RUST_LOG=none target/debug/monolake -c benchmark/proxy/monolake/monolake.toml &
