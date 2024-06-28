if [ -z "${MONOLAKE_HOME+set}" ]; then
    export MONOLAKE_HOME=$HOME/monolake
fi

cd $MONOLAKE_HOME/client

# download curl
wget https://curl.se/download/curl-8.3.0.zip
unzip curl-8.3.0.zip
cd curl-8.3.0
./configure --prefix=$HOME/curl --with-openssl
make
sudo make install

# download wrk2
git clone https://github.com/giltene/wrk2
cd wrk2
make WITH_OPENSSL=/usr
