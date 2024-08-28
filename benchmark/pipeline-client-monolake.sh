export client_url=3.133.229.116
export proxy_url=18.217.152.113
export server_url=3.22.140.218
export proxy_private_url=172.31.2.253
export server_private_url=172.31.22.170

# start client
client_cmd='cd ~/monolake/benchmark/client; export MONOLAKE_BENCHMARK_PROXY_IP='
client_cmd+=$proxy_private_url
client_cmd+='; export MONOLAKE_BENCHMARK_SERVER_IP='
client_cmd+=$server_private_url
client_cmd+='; ./benchmark-monolake-http.sh; ./benchmark-monolake-https.sh; echo "Please type exit to continue"; bash -l'
ssh -i $HOME/ssh/monolake-benchmark.pem ec2-user@${client_url} -t $client_cmd
