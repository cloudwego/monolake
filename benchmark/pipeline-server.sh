export client_url=3.133.229.116
export proxy_url=3.19.41.190
export server_url=3.22.140.218
export proxy_private_url=172.31.7.16
export server_private_url=172.31.22.170

# start server
ssh -i $HOME/ssh/monolake-benchmark.pem ec2-user@${server_url} -t 'sudo service nginx restart; sleep 3; sudo rm -f nginx-performance.csv; sudo ~/monolake/benchmark/performance-collect.sh nginx; echo "Please type exit to continue"; bash -l'
