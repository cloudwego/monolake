# new_terminal=`osascript -e 'tell app "Terminal" to do script $1'`
# new_terminal='gnome-terminal -- $1'

export client_url=3.133.229.116
export proxy_url=3.19.41.190
export server_url=3.22.140.218
export proxy_private_url=172.31.7.16
export server_private_url=172.31.22.170

# manual update proxy configurations
echo "make sure proxy configurations are updated manually"
# ssh -i $HOME/ssh/monolake-benchmark.pem ec2-user@${proxy_url} -t 'cd ~/monolake/benchmark/proxy; MONOLAKE_BENCHMARK_SERVER_IP=${server_url} ./update-server-ip.sh; bash -l'

# start server
echo "start server"
osascript -e 'tell app "Terminal" to do script "~/code/monolake/benchmark/pipeline-server.sh"'
sleep 5

# then start proxy monolake
echo "start proxy monolake"
osascript -e 'tell app "Terminal" to do script "~/code/monolake/benchmark/pipeline-proxy-monolake.sh"'
sleep 5

# start client
echo "start client"
osascript -e 'tell app "Terminal" to do script "~/code/monolake/benchmark/pipeline-client-monolake.sh"'
sleep 2

echo "start client-metrics-collect"
ssh -i $HOME/ssh/monolake-benchmark.pem ec2-user@${client_url} -t 'cd monolake/benchmark; ~/monolake/benchmark/performance-collect.sh wrk; ~/monolake/benchmark/performance-collect.sh wrk; ~/monolake/benchmark/performance-collect.sh wrk; ~/monolake/benchmark/performance-collect.sh wrk; ~/monolake/benchmark/performance-collect.sh wrk; ~/monolake/benchmark/performance-collect.sh wrk; ~/monolake/benchmark/performance-collect.sh wrk; ~/monolake/benchmark/performance-collect.sh wrk; echo "Please type exit to continue"; bash -l'

#stop proxy monolake
echo "stop proxy monolake"
ssh -i $HOME/ssh/monolake-benchmark.pem ec2-user@${proxy_url} -t 'cd ~/monolake/benchmark/proxy; ./stop-monolake.sh'
sleep 2

#stop server
echo "stop server"
ssh -i $HOME/ssh/monolake-benchmark.pem ec2-user@${server_url} -t 'sudo service nginx stop'
sleep 2

echo "visualize"
cd visualization/

# copy collected data from client
echo "copy collected data from client"
# scp -i $HOME/ssh/monolake-benchmark.pem ec2-user@${client_url}:wrk-performance.csv .
scp -i $HOME/ssh/monolake-benchmark.pem ec2-user@${client_url}:"wrk2/*.txt" .

#copy collected data from server
echo "copy collected data from server"
# scp -i $HOME/ssh/monolake-benchmark.pem ec2-user@${server_url}:nginx-performance.csv ./server-performance.csv

#copy collected data from proxy
echo "copy collected data from proxy"
# scp -i $HOME/ssh/monolake-benchmark.pem ec2-user@${proxy_url}:nginx-performance.csv .
# scp -i $HOME/ssh/monolake-benchmark.pem ec2-user@${proxy_url}:traefik-performance.csv .
scp -i $HOME/ssh/monolake-benchmark.pem ec2-user@${proxy_url}:monolake-performance.csv .

#plot data
echo "plot data"
# ./performance-plot.sh nginx
# ./performance-plot.sh traefik
./performance-plot.sh monolake
./performance-plot.sh server
# ./performance-plot.sh wrk
# ./nginx-http-latency-plot.sh
# ./traefik-http-latency-plot.sh
./monolake-http-latency-plot.sh
./all-http-latency-plot.sh
# ./nginx-https-latency-plot.sh
# ./traefik-https-latency-plot.sh
./monolake-https-latency-plot.sh
./all-https-latency-plot.sh
