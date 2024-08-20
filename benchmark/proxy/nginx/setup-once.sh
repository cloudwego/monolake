# setup nginx proxy

if [ -z "${MONOLAKE_HOME+set}" ]; then
    export MONOLAKE_HOME=$HOME/monolake
fi

# build nginx
# sudo yum -y install pcre pcre-devel zlib-devel
# wget https://nginx.org/download/nginx-1.27.1.tar.gz
# tar -zxvf nginx-1.27.1.tar.gz
# cd nginx-1.27.1
# ./configure --with-http_ssl_module
# make
# sudo make install
# installed to: /usr/local/nginx/sbin/nginx 

sudo yum install -y nginx
sudo openssl req -x509 -nodes -days 365 -newkey rsa:2048 -keyout /etc/nginx/cert.key -out /etc/nginx/cert.pem
sudo cat /etc/nginx/cert.key /etc/nginx/cert.pem > $MONOLAKE_HOME/combined.pem
sudo mv $MONOLAKE_HOME/combined.pem /etc/nginx/
sudo cp /etc/nginx/cert.key $MONOLAKE_HOME/examples/certs/key.pem
sudo cp /etc/nginx/cert.pem $MONOLAKE_HOME/examples/certs/
sudo chmod 777 $MONOLAKE_HOME/examples/certs/key.pem $MONOLAKE_HOME/examples/certs/cert.pem
