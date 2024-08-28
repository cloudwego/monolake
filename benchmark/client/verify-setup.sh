# verify if proxy and server are ready and running; run this dcript from client

#export MONOLAKE_BENCHMARK_PROXY_IP=ec2-52-15-182-194.us-east-2.compute.amazonaws.com
#export MONOLAKE_BENCHMARK_SERVER_IP=ec2-3-129-244-251.us-east-2.compute.amazonaws.com

if [ -z "${MONOLAKE_BENCHMARK_PROXY_IP+set}" ]; then
    export MONOLAKE_BENCHMARK_PROXY_IP=localhost
fi

if [ -z "${MONOLAKE_BENCHMARK_SERVER_IP+set}" ]; then
    export MONOLAKE_BENCHMARK_SERVER_IP=localhost
fi

# verify server is ready
curl -k http://$MONOLAKE_BENCHMARK_SERVER_IP:10082

# verify (nginx) proxy is ready
curl -k http://$MONOLAKE_BENCHMARK_PROXY_IP:8100/server2
# curl -k http://$MONOLAKE_BENCHMARK_PROXY_IP:8200
# verify (traefik) proxy is ready
curl -k http://$MONOLAKE_BENCHMARK_PROXY_IP:8300/server2
# verify (monolake) proxy is ready
curl -k http://$MONOLAKE_BENCHMARK_PROXY_IP:8402
# verify (envoy) proxy is ready
curl -k http://$MONOLAKE_BENCHMARK_PROXY_IP:8500/server2/

# verify (nginx) tls proxy is ready
curl -k https://$MONOLAKE_BENCHMARK_PROXY_IP:8443/server2
# curl -k https://$MONOLAKE_BENCHMARK_PROXY_IP:7443
# verify (traefik) tls proxy is ready
curl -k https://$MONOLAKE_BENCHMARK_PROXY_IP:9443/server2
# verify (monolake) tls proxy is ready
curl -k https://$MONOLAKE_BENCHMARK_PROXY_IP:6442
# verify (envoy) tls proxy is ready
curl -k https://$MONOLAKE_BENCHMARK_PROXY_IP:5443/server2/
