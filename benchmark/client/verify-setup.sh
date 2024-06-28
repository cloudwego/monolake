# verify if proxy and server are ready and running; run this dcript from client

if [ -z "${MONOLAKE_BENCHMARK_PROXY_IP+set}" ]; then
    export MONOLAKE_BENCHMARK_PROXY_IP=localhost
fi

if [ -z "${MONOLAKE_BENCHMARK_SERVER_IP+set}" ]; then
    export MONOLAKE_BENCHMARK_SERVER_IP=localhost
fi

# verify server is ready
curl -k http://$MONOLAKE_BENCHMARK_SERVER_IP

# verify server tls is ready
curl -k https://$MONOLAKE_BENCHMARK_SERVER_IP

# verify proxy is ready
curl -k http://$MONOLAKE_BENCHMARK_PROXY_IP:8000

# verify proxy tls is ready
curl -k https://$MONOLAKE_BENCHMARK_PROXY_IP:6443
