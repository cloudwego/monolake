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

# https proxy for monolake
./wrk -d 1m -c 640 -t 64 -R 160000 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:6442 > https-result-4c-monolake-tiny.txt
./wrk -d 1m -c 640 -t 64 -R 140000 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:6443 > https-result-4c-monolake-small.txt
./wrk -d 1m -c 640 -t 64 -R 80000 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:6444 > https-result-4c-monolake-medium.txt
./wrk -d 1m -c 640 -t 64 -R 9000 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:6445 > https-result-4c-monolake-large.txt

# ./wrk -d 1m -c 1447 -t 20 -R 70000 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:6442 > https-result-4c-monolake-tiny.txt
# ./wrk -d 1m -c 1447 -t 20 -R 62000 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:6443 > https-result-4c-monolake-small.txt
# ./wrk -d 1m -c 1447 -t 20 -R 60000 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:6444 > https-result-4c-monolake-medium.txt
# ./wrk -d 1m -c 1447 -t 20 -R 6000 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:6445 > https-result-4c-monolake-large.txt

# ./wrk -d 1m -c 1000 -t 20 -R 2000 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:6442 > https-result-4c-monolake-tiny.txt
# ./wrk -d 1m -c 1000 -t 20 -R 2000 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:6443 > https-result-4c-monolake-small.txt
# ./wrk -d 1m -c 1000 -t 20 -R 2000 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:6444 > https-result-4c-monolake-medium.txt
# ./wrk -d 1m -c 1000 -t 20 -R 2000 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:6445 > https-result-4c-monolake-large.txt

# https proxy for haproxy (not used)
# ./wrk -d 1m -c 1000 -t 20 -R 2000 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:9443/server2 > https-result-4c-haproxy-tiny.txt
# ./wrk -d 1m -c 1000 -t 20 -R 2000 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:9443/server3 > https-result-4c-haproxy-small.txt
# ./wrk -d 1m -c 1000 -t 20 -R 2000 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:9443/server4 > https-result-4c-haproxy-medium.txt
# ./wrk -d 1m -c 1000 -t 20 -R 2000 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:9443/server5 > https-result-4c-haproxy-large.txt
