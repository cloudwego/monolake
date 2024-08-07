# run benchmark: make sure proxy and server all are running; run this script from client
if [ -z "${MONOLAKE_HOME+set}" ]; then
    export MONOLAKE_HOME=$HOME/monolake
fi

if [ -z "${MONOLAKE_BENCHMARK_PROXY_IP+set}" ]; then
    export MONOLAKE_BENCHMARK_PROXY_IP=localhost
fi

if [ -z "${MONOLAKE_BENCHMARK_SERVER_IP+set}" ]; then
    export MONOLAKE_BENCHMARK_SERVER_IP=localhost
fi

cd $HOME/wrk2

# https proxy for traefik
./wrk -d 1m -c 1447 -t 20 -R 1800 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:9443/server2 > https-result-4c-traefik-tiny.txt
./wrk -d 1m -c 1447 -t 20 -R 1800 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:9443/server3 > https-result-4c-traefik-small.txt
./wrk -d 1m -c 1447 -t 20 -R 2800 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:9443/server4 > https-result-4c-traefik-medium.txt
./wrk -d 1m -c 1200 -t 20 -R 3600 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:9443/server5 > https-result-4c-traefik-large.txt

# ./wrk -d 1m -c 1500 -t 20 -R 20000 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:9443/server2 > https-result-4c-traefik-tiny.txt
# ./wrk -d 1m -c 1500 -t 20 -R 20000 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:9443/server3 > https-result-4c-traefik-small.txt
# ./wrk -d 1m -c 1500 -t 20 -R 20000 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:9443/server4 > https-result-4c-traefik-medium.txt
# ./wrk -d 1m -c 1500 -t 20 -R 20000 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:9443/server5 > https-result-4c-traefik-large.txt
