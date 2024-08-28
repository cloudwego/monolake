# setup envoy proxy

if [ -z "${MONOLAKE_HOME+set}" ]; then
    export MONOLAKE_HOME=$HOME/monolake
fi

cd $MONOLAKE_HOME/benchmark/proxy/envoy
wget https://github.com/envoyproxy/envoy/releases/download/v1.31.0/envoy-1.31.0-linux-x86_64
chmod +x envoy-1.31.0-linux-x86_64
mv envoy-1.31.0-linux-x86_64 envoy
echo "Please fill all fields when generating OpenSSL certs."
sudo openssl req -x509 -nodes -days 365 -newkey rsa:2048 -keyout $MONOLAKE_HOME/benchmark/proxy/envoy/cert.key -out $MONOLAKE_HOME/benchmark/proxy/envoy/cert.pem
