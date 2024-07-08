# replace server ip in proxy services config file

if [ -z "${MONOLAKE_HOME+set}" ]; then
    export MONOLAKE_HOME=$HOME/monolake
fi

if [ -z "${MONOLAKE_BENCHMARK_PROXY_IP+set}" ]; then
    export MONOLAKE_BENCHMARK_PROXY_IP=localhost
fi

if [ -z "${MONOLAKE_BENCHMARK_SERVER_IP+set}" ]; then
    export MONOLAKE_BENCHMARK_SERVER_IP=localhost
fi

cd $MONOLAKE_HOME/benchmark/proxy
echo "please copy and paste following commands and run manually to replace server ip in proxy services config file"
echo "sed -i -e 's/127.0.0.1/${MONOLAKE_BENCHMARK_SERVER_IP}/g' nginx/nginx.conf"
echo "sed -i -e 's/127.0.0.1/${MONOLAKE_BENCHMARK_SERVER_IP}/g' monolake/monolake.toml"
echo "sed -i -e 's/127.0.0.1/${MONOLAKE_BENCHMARK_SERVER_IP}/g' traefik/traefik-dynamic.toml"
