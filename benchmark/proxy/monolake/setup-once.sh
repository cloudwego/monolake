# setup monolake proxy

if [ -z "${MONOLAKE_HOME+set}" ]; then
    export MONOLAKE_HOME=$HOME/monolake
fi

cd $HOME

sudo yum -y install gcc openssl-devel

# install rust nightly
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
. "$HOME/.cargo/env"
rustup toolchain install nightly
rustup default nightly

cd $MONOLAKE_HOME

# generate certs
sh -c "cd examples && ./gen_cert.sh"
mkdir -p examples/certs && openssl req -x509 -newkey rsa:2048 -keyout examples/certs/key.pem -out examples/certs/cert.pem -sha256 -days 365 -nodes -subj "/CN=monolake.cloudwego.io"

# build monolake
cd $MONOLAKE_HOME
cargo build --release
