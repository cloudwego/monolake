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
cargo build
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
