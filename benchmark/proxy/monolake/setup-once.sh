# setup monolake proxy

if [ -z "${MONOLAKE_HOME+set}" ]; then
    export MONOLAKE_HOME=$HOME/monolake
fi

cd $MONOLAKE_HOME
cp $MONOLAKE_HOME/benchmark/proxy/monolake/monolake.toml monolake/examples/
cd monolake
mkdir examples/certs && openssl req -x509 -newkey rsa:2048 -keyout examples/certs/key.pem -out examples/certs/cert.pem -sha256 -days 365 -nodes -subj "/CN=monolake.cloudwego.io"
cargo build
