export client_url=ec2-18-116-241-44.us-east-2.compute.amazonaws.com
export proxy_url=ec2-18-226-87-157.us-east-2.compute.amazonaws.com
export server_url=ec2-3-133-91-193.us-east-2.compute.amazonaws.com

#manual update proxy configurations
#ssh -i $HOME/ssh/monolake-benchmark.pem ec2-user@${proxy_url} -t 'cd ~/monolake/benchmark/proxy; MONOLAKE_BENCHMARK_SERVER_IP=${server_url} ./update-server-ip.sh; bash -l'

#then start proxy
proxy_cmd='export MONOLAKE_BENCHMARK_PROXY_IP='
proxy_cmd+=$proxy_url
proxy_cmd+='; export MONOLAKE_BENCHMARK_SERVER_IP='
proxy_cmd+=$server_url
proxy_cmd+='; ~/monolake/benchmark/proxy/start-monolake.sh; sleep 3; rm -f ~/monolake-performance.csv; ~/monolake/benchmark/performance-collect.sh monolake; echo "Please type exit to continue"; bash -l'
ssh -i $HOME/ssh/monolake-benchmark.pem ec2-user@${proxy_url} -t $proxy_cmd
