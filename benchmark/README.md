# Monolake Benchmark

Monolake benchmark contains programs and scripts to benchmark monolake's performance and comparison with other popular open source proxy programs.

## Topolonogy

A client and a server will be setup on separated machines and traffic will go through the proxy which will be on another machine. The client machine is more powerful so that when testing it can reach or over the capabilities of the server. All machines are on a stable network environment to flat traffic varies.

Basic test tool is wrk2 and it is installed on client machine. Nginx will be setup on server machine as web backend. Different proxy programs (monolake, nginx, traefik) will be installed and tested to compare.

![network-topology](images/README/network-topology.png)

We plan to benchmark monolake for 2 cases. The first case is monolake on a 4 core machine as proxy, another case is monolake on a 16 core machine.

## Reproduce on AWS EC2 machines

Reference aws ec2 id for client machine: standard aws linux image on c6a.8xlarge

Reference aws ec2 id for server machine: standard aws linux image on c6a.2xlarge

The server machine should be configured with some security group like this:

![server-security-group](images/README/server-security-group.png)

Reference aws ec2 id for proxy service machine: standard aws linux image on c5a.xlarge (4 cores) and c6a.4xlarge (16 cores)

The proxy machine should be configured with some security group like this:

![proxy-security-group](images/README/proxy-security-group.png)

## Setup

### Client Setup

client/setup-once.sh will be used to install test tools on the client machine: curl, wrk2.

```bash
cd $MONOLAKE_HOME/client
sudo yum -y install gcc git openssl-devel zlib-devel

# download curl: it is installed by default

# download wrk2
cd $HOME
git clone https://github.com/giltene/wrk2
cd wrk2
make WITH_OPENSSL=/usr
```

### Server Setup

server/setup-once.sh will be used to install nginx web server on the server machine.

```bash
sudo yum -y install nginx
sudo mv /etc/nginx/nginx.conf /etc/nginx/nginx-original.conf
sudo cp $MONOLAKE_HOME/benchmark/server/nginx-web.conf /etc/nginx/nginx.conf
sudo openssl req -x509 -nodes -days 365 -newkey rsa:2048 -keyout /etc/nginx/cert.key -out /etc/nginx/cert.pem
sudo cat /etc/nginx/cert.key /etc/nginx/cert.pem > $MONOLAKE_HOME/combined.pem
sudo mv $MONOLAKE_HOME/combined.pem /etc/nginx/
sudo cp -r $MONOLAKE_HOME/benchmark/server/webroot/* /usr/share/nginx/html/
sudo service nginx restart
```

### Proxy Setup

proxy/<>/setup-once.sh will be used to install proxy softwares monolake and comparisons nginx and traefik on the proxy machine.

#### proxy/monolake/setup-once.sh

```bash
sudo yum -y install gcc openssl-devel

# install rust nightly
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

cd $MONOLAKE_HOME

# generate certs
sh -c "cd examples && ./gen_cert.sh"
mkdir examples/certs && openssl req -x509 -newkey rsa:2048 -keyout examples/certs/key.pem -out examples/certs/cert.pem -sha256 -days 365 -nodes -subj "/CN=monolake.cloudwego.io"

# build monolake
cd $MONOLAKE_HOME
cargo build --release
```

#### proxy/nginx/setup-once.sh

```bash
sudo yum install -y nginx
sudo openssl req -x509 -nodes -days 365 -newkey rsa:2048 -keyout /etc/nginx/cert.key -out /etc/nginx/cert.pem
sudo cat /etc/nginx/cert.key /etc/nginx/cert.pem > $MONOLAKE_HOME/combined.pem
sudo mv $MONOLAKE_HOME/combined.pem /etc/nginx/
```

#### proxy/traefik/setup-once.sh

```bash
cd $MONOLAKE_HOME/benchmark/proxy/traefik/
wget https://github.com/traefik/traefik/releases/download/v3.0.0-rc1/traefik_v3.0.0-rc1_linux_amd64.tar.gz
tar zxvf traefik_v3.0.0-rc1_linux_amd64.tar.gz
rm traefik_v3.0.0-rc1_linux_amd64.tar.gz
```

### proxy/envoy/setup-once.sh

```bash
cd $MONOLAKE_HOME/benchmark/proxy/envoy
wget https://github.com/envoyproxy/envoy/releases/download/v1.31.0/envoy-1.31.0-linux-x86_64
chmod +x envoy-1.31.0-linux-x86_64
mv envoy-1.31.0-linux-x86_64 envoy
echo "Please fill all fields when generating OpenSSL certs."
sudo openssl req -x509 -nodes -days 365 -newkey rsa:2048 -keyout $MONOLAKE_HOME/benchmark/proxy/envoy/cert.key -out $MONOLAKE_HOME/benchmark/proxy/envoy/cert.pem
```

### Configure Server IP

proxy/update-server-ip.sh contain scripts to update server ip in the proxy configure files. But it must be copy&pasted to console with replacing ${MONOLAKE_BENCHMARK_SERVER_IP} with real url, then run manually. Sed does not support environment variables and directly run the script will not result expected replacement.

```bash
cd $MONOLAKE_HOME/benchmark/proxy
sed -i -e 's/127.0.0.1/${MONOLAKE_BENCHMARK_SERVER_IP}/g' nginx/nginx.conf
sed -i -e 's/127.0.0.1/${MONOLAKE_BENCHMARK_SERVER_IP}/g' monolake/monolake.toml
sed -i -e 's/127.0.0.1/${MONOLAKE_BENCHMARK_SERVER_IP}/g' traefik/traefik-dynamic.toml
```

### Runtime Environment Variables

```bash
if [ -z "${MONOLAKE_HOME+set}" ]; then
    export MONOLAKE_HOME=$HOME/monolake
fi

if [ -z "${MONOLAKE_BENCHMARK_PROXY_IP+set}" ]; then
    export MONOLAKE_BENCHMARK_PROXY_IP=localhost
fi

if [ -z "${MONOLAKE_BENCHMARK_SERVER_IP+set}" ]; then
    export MONOLAKE_BENCHMARK_SERVER_IP=localhost
fi
```

## Run Benchmark Test

Normally we run setup-once.sh on each machine first. For proxy machine, we only need run required 1 of 3 proxy services and don't run the other 2. Also we need run update-server-ip.sh.

Now we need make sure the setup is ready. On the client, we set environment variable MONOLAKE_BENCHMARK_SERVER_IP by:

`export MONOLAKE_BENCHMARK_SERVER_IP=<server-ip>`

then run:

```bash
client/verify.sh
```

to make sure the result is expected.

We can run benchmark test for different proxy service. For example, to benchmark monolake proxy service for http:

```bash
client/benchmark-monolake-http.sh
```

Before run the benchmark, make sure MONOLAKE_BENCHMARK_SERVER_IP and MONOLAKE_BENCHMARK_PROXY_IP are set correctly.

Check connections for down stream and up stream connections:

```bash
netstat -tn | grep ESTAB | grep <down/up-stream-ip> | wc -l
```

## Visualize the result

### Collect the data

#### Collect performance data

Run performance-collect.sh on the machine which need performance data. The script can be run on client, proxy and server. For example

```bash
./performance-collect.sh wrk # client
./performance-collect.sh monolake # proxy
./performance-collect.sh nginx # server
```

#### Collect latency data

When we run benchmark using wrk2, the latency data is already generated and saved to local files.

### Plot the data

gnuplot is used to plot the data and the results are in .png image format. gnuplot needs to be installed. User may also copy the data to another machine with gnuplot installed, and plot the result.

#### Plot performance data

performance-plot.sh will be used to plot the performance data. The results are 4 .png image files: cpu-mem-usage-<process>.png, tcp-count-<process>.png, performance-metrices-<process>.png, thread-count-<process>.png. The script can be run for client, proxy and server. For example

```bash
./performance-plot.sh wrk # client
./performance-plot.sh monolake # proxy
./performance-plot.sh nginx # server
```

#### Plot latency data

There are some scripts to plot latency data in visualization/ directory. For example

```bash
./monolake-http-latency-plot.sh
./monolake-https-latency-plot.sh
./all-http-latency-plot.sh
```

After running the scripts the results are in .png image format.

## Pipeline/Automation

We can simplify the test to pipeline/automation scripts.

Some steps are manual steps.

* Correct URLs/IPs: replace in all benchmark-pipeline-xxx.sh and pipeline-xxx.sh
* Avoid ssh access prompt: ssh to client/proxy service/server once
* Setup: running setup-once.sh in the directories
* Update server-ip in the configuration files: running benchmark/proxy/update-server-ip.sh and follow the sed commands

```bash
export client_url=<client-eip>
export proxy_url=<proxy-eip>
export server_url=<server-eip>
export proxy_private_url=<proxy-private-ip>
export server_private_url=<server-private-ip>
```

Then user can use benchmark-pipeline.sh to run all test in one script. User may need type "exit" to quit some finished jobs and go to the next step. User can early input "exit" when "Writing data to CSV file wrk-performance.csv..." prompts. Finally, user will get the results and visualized images.

Pipeline scripts contain plot scripts, so it is better to run on a host machine with GUI. Following pipeline scripts runs on OS X host and gnuplot is installed on it. If user wants to run pipeline scripts on linux/ubuntu host, please install gnuplot and use gnome-terminal as termainal tool. User may directly use non pipeline scripts on AWS EC2 linux machines.

```bash
# new_terminal=`osascript -e 'tell app "Terminal" to do script $1'`
# new_terminal='gnome-terminal -- $1'

export client_url=<client-eip>
export proxy_url=<proxy-eip>
export server_url=<server-eip>
export proxy_private_url=<proxy-private-ip>
export server_private_url=<server-private-ip>

# manual update proxy configurations
echo "make sure proxy configurations are updated manually"
# ssh -i $HOME/ssh/monolake-benchmark.pem ec2-user@${proxy_url} -t 'cd ~/monolake/benchmark/proxy; MONOLAKE_BENCHMARK_SERVER_IP=${server_url} ./update-server-ip.sh; bash -l'

# start server
echo "start server"
osascript -e 'tell app "Terminal" to do script "~/code/monolake/benchmark/pipeline-server.sh"'
sleep 5

# then start proxy nginx
echo "start proxy nginx"
osascript -e 'tell app "Terminal" to do script "~/code/monolake/benchmark/pipeline-proxy-nginx.sh"'
sleep 5

ssh -i $HOME/ssh/monolake-benchmark.pem ec2-user@${client_url} -t 'rm -f monolake/benchmark/wrk-performance.csv'

# start client nginx
echo "start client nginx"
osascript -e 'tell app "Terminal" to do script "~/code/monolake/benchmark/pipeline-client-nginx.sh"'
sleep 2

echo "start client-metrics-collect"
ssh -i $HOME/ssh/monolake-benchmark.pem ec2-user@${client_url} -t 'cd monolake/benchmark; ~/monolake/benchmark/performance-collect.sh wrk; ~/monolake/benchmark/performance-collect.sh wrk; ~/monolake/benchmark/performance-collect.sh wrk; ~/monolake/benchmark/performance-collect.sh wrk; ~/monolake/benchmark/performance-collect.sh wrk; ~/monolake/benchmark/performance-collect.sh wrk; ~/monolake/benchmark/performance-collect.sh wrk; ~/monolake/benchmark/performance-collect.sh wrk; echo "Please type exit to continue"; bash -l'

#stop proxy nginx
echo "stop proxy nginx"
ssh -i $HOME/ssh/monolake-benchmark.pem ec2-user@${proxy_url} -t 'cd ~/monolake/benchmark/proxy; ./stop-nginx.sh'
sleep 2

# then start proxy traefik
echo "start proxy traefik"
osascript -e 'tell app "Terminal" to do script "~/code/monolake/benchmark/pipeline-proxy-traefik.sh"'
sleep 5

# start client traefik
echo "start client"
osascript -e 'tell app "Terminal" to do script "~/code/monolake/benchmark/pipeline-client-traefik.sh"'
sleep 2

echo "start client-metrics-collect"
ssh -i $HOME/ssh/monolake-benchmark.pem ec2-user@${client_url} -t 'cd monolake/benchmark; ~/monolake/benchmark/performance-collect.sh wrk; ~/monolake/benchmark/performance-collect.sh wrk; ~/monolake/benchmark/performance-collect.sh wrk; ~/monolake/benchmark/performance-collect.sh wrk; ~/monolake/benchmark/performance-collect.sh wrk; ~/monolake/benchmark/performance-collect.sh wrk; ~/monolake/benchmark/performance-collect.sh wrk; ~/monolake/benchmark/performance-collect.sh wrk; echo "Please type exit to continue"; bash -l'

#stop proxy traefik
echo "stop proxy traefik"
ssh -i $HOME/ssh/monolake-benchmark.pem ec2-user@${proxy_url} -t 'cd ~/monolake/benchmark/proxy; ./stop-traefik.sh'
sleep 2

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

# then start proxy envoy
echo "start proxy envoy"
osascript -e 'tell app "Terminal" to do script "~/code/monolake/benchmark/pipeline-proxy-envoy.sh"'
sleep 5

# start client
echo "start client"
osascript -e 'tell app "Terminal" to do script "~/code/monolake/benchmark/pipeline-client-envoy.sh"'
sleep 2

echo "start client-metrics-collect"
ssh -i $HOME/ssh/monolake-benchmark.pem ec2-user@${client_url} -t 'cd monolake/benchmark; ~/monolake/benchmark/performance-collect.sh wrk; ~/monolake/benchmark/performance-collect.sh wrk; ~/monolake/benchmark/performance-collect.sh wrk; ~/monolake/benchmark/performance-collect.sh wrk; ~/monolake/benchmark/performance-collect.sh wrk; ~/monolake/benchmark/performance-collect.sh wrk; ~/monolake/benchmark/performance-collect.sh wrk; ~/monolake/benchmark/performance-collect.sh wrk; echo "Please type exit to continue"; bash -l'

#stop proxy envoy
echo "stop proxy envoy"
ssh -i $HOME/ssh/monolake-benchmark.pem ec2-user@${proxy_url} -t 'cd ~/monolake/benchmark/proxy; ./stop-envoy.sh'
sleep 2

#stop server
echo "stop server"
ssh -i $HOME/ssh/monolake-benchmark.pem ec2-user@${server_url} -t 'sudo service nginx stop'
sleep 2

echo "visualize"
cd visualization/

# copy collected data from client
echo "copy collected data from client"
scp -i $HOME/ssh/monolake-benchmark.pem ec2-user@${client_url}:wrk-performance.csv .
scp -i $HOME/ssh/monolake-benchmark.pem ec2-user@${client_url}:"wrk2/*.txt" .

#copy collected data from server
echo "copy collected data from server"
scp -i $HOME/ssh/monolake-benchmark.pem ec2-user@${server_url}:nginx-performance.csv ./server-performance.csv

#copy collected data from proxy
echo "copy collected data from proxy"
scp -i $HOME/ssh/monolake-benchmark.pem ec2-user@${proxy_url}:nginx-performance.csv .
scp -i $HOME/ssh/monolake-benchmark.pem ec2-user@${proxy_url}:traefik-performance.csv .
scp -i $HOME/ssh/monolake-benchmark.pem ec2-user@${proxy_url}:monolake-performance.csv .
scp -i $HOME/ssh/monolake-benchmark.pem ec2-user@${proxy_url}:envoy-performance.csv .

#plot data
echo "plot data"
./performance-plot.sh nginx
./performance-plot.sh traefik
./performance-plot.sh monolake
./performance-plot.sh envoy
./performance-plot.sh server
./performance-plot.sh wrk
./nginx-http-latency-plot.sh
./traefik-http-latency-plot.sh
./monolake-http-latency-plot.sh
./envoy-http-latency-plot.sh
./all-http-latency-plot.sh
./nginx-https-latency-plot.sh
./traefik-https-latency-plot.sh
./monolake-https-latency-plot.sh
./envoy-https-latency-plot.sh
./all-https-latency-plot.sh
./proxies-performance-plot.sh
```

The visualized result example:

Proxy service/monolake system performance:

![performance-metrices-monolake](images/README/performance-metrices-monolake.png)

Client/wrk2 http through proxy services latency:

![all-http-latency](images/README/all-http-latency.png)

Client/wrk2 https through proxy services latency:

![all-latency-https](images/README/all-latency-https.png)

Throughput and requests per second compare:


| Case                            | Requests/sec | Transfer/sec  | Server Error | Timeout |
| ------------------------------- | ------------ | ------------- | ------------ | ------- |
| http-result-4c-monolake-tiny    |    161101.29 |   54934896.64 |            0 |       0 |
| http-result-4c-monolake-small   |    151977.67 |  529488936.96 |            0 |       0 |
| http-result-4c-monolake-medium  |     85893.56 |  987842478.08 |            0 |       0 |
| http-result-4c-monolake-large   |      9186.06 | 1524713390.08 |            0 |     618 |
| http-result-4c-nginx-tiny       |    187973.44 |   68608327.68 |            0 |       0 |
| http-result-4c-nginx-small      |    176318.84 |  618523525.12 |            0 |       0 |
| http-result-4c-nginx-medium     |    108853.56 | 1256277934.08 |            0 |       0 |
| http-result-4c-nginx-large      |      9304.14 | 1546188226.56 |            0 |      22 |
| http-result-4c-traefik-tiny     |      9991.09 |    3407872.00 |            0 |       0 |
| http-result-4c-traefik-small    |     10989.34 |   38283509.76 |            0 |       0 |
| http-result-4c-traefik-medium   |     11988.52 |  137688514.56 |            0 |       0 |
| http-result-4c-traefik-large    |      8737.57 | 1449551462.40 |            0 |       0 |
| http-result-4c-envoy-tiny       |     36951.75 |   13600030.72 |            0 |       0 |
| http-result-4c-envoy-small      |     35367.03 |  124182855.68 |            0 |       0 |
| http-result-4c-envoy-medium     |     29637.89 |  341206630.40 |            0 |       0 |
| http-result-4c-envoy-large      |      9285.01 | 1546188226.56 |            0 |      46 |
| https-result-4c-monolake-tiny   |    141883.70 |   48381296.64 |            0 |       0 |
| https-result-4c-monolake-small  |    116831.85 |  407046717.44 |            0 |       0 |
| https-result-4c-monolake-medium |     63390.91 |  728047288.32 |            0 |       0 |
| https-result-4c-monolake-large  |      7946.21 | 1320702443.52 |            0 |       0 |
| https-result-4c-nginx-tiny      |    127167.08 |   46420459.52 |            0 |       0 |
| https-result-4c-nginx-small     |    114350.27 |  401143234.56 |            0 |       0 |
| https-result-4c-nginx-medium    |     62450.58 |  718746419.20 |            0 |       0 |
| https-result-4c-nginx-large     |      7881.00 | 1309965025.28 |            0 |      15 |
| https-result-4c-traefik-tiny    |      9943.28 |    3386900.48 |            0 |       0 |
| https-result-4c-traefik-small   |     11888.59 |   41418752.00 |            0 |       0 |
| https-result-4c-traefik-medium  |     13914.15 |  159802982.40 |            0 |       0 |
| https-result-4c-traefik-large   |      7698.61 | 1277752770.56 |            0 |       0 |
| https-result-4c-envoy-tiny      |     34158.00 |   12582912.00 |            0 |       0 |
| https-result-4c-envoy-small     |     33054.16 |  116066877.44 |            0 |       0 |
| https-result-4c-envoy-medium    |     26968.24 |  310472867.84 |            0 |       0 |
| https-result-4c-envoy-large     |      8349.09 | 1385126952.96 |            0 |       0 |

![proxies-performance](images/README/proxies-performance.png)

Throughput and requests per second compare by payload size:


| Case                     | Tiny Requests/sec | Small Requests/sec | Medium Requests/sec | Large Requests/sec | Tiny Transfer/sec | Small Transfer/sec | Medium Transfer/sec | Large Transfer/sec |
| ------------------------ | ----------------- | ------------------ | ------------------- | ------------------ | ----------------- | ------------------ | ------------------- | ------------------ |
| http-result-4c-monolake  |         161101.29 |          151977.67 |            85893.56 |            9186.06 |       54934896.64 |       529488936.96 |        987842478.08 |      1524713390.08 |
| http-result-4c-nginx     |         187973.44 |          176318.84 |           108853.56 |            9304.14 |       68608327.68 |       618523525.12 |       1256277934.08 |      1546188226.56 |
| http-result-4c-traefik   |           9991.09 |           10989.34 |            11988.52 |            8737.57 |        3407872.00 |        38283509.76 |        137688514.56 |      1449551462.40 |
| http-result-4c-envoy     |          36951.75 |           35367.03 |            29637.89 |            9285.01 |       13600030.72 |       124182855.68 |        341206630.40 |      1546188226.56 |
| https-result-4c-monolake |         141883.70 |          116831.85 |            63390.91 |            7946.21 |       48381296.64 |       407046717.44 |        728047288.32 |      1320702443.52 |
| https-result-4c-nginx    |         127167.08 |          114350.27 |            62450.58 |            7881.00 |       46420459.52 |       401143234.56 |        718746419.20 |      1309965025.28 |
| https-result-4c-traefik  |           9943.28 |           11888.59 |            13914.15 |            7698.61 |        3386900.48 |        41418752.00 |        159802982.40 |      1277752770.56 |
| https-result-4c-envoy    |          34158.00 |           33054.16 |            26968.24 |            8349.09 |       12582912.00 |       116066877.44 |        310472867.84 |      1385126952.96 |

![proxies-performance-rotated](images/README/proxies-performance-rotated.png)
