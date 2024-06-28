# start nginx server

if [ -z "${MONOLAKE_HOME+set}" ]; then
    export MONOLAKE_HOME=$HOME/monolake
fi

sudo apt install -y nginx
sudo cp $MONOLAKE_HOME/benchmark/server/nginx-web.conf /etc/nginx/
sudo openssl req -x509 -nodes -days 365 -newkey rsa:2048 -keyout /etc/nginx/cert.key -out /etc/nginx/cert.pem
sudo cat /etc/nginx/cert.key /etc/nginx/cert.pem > $MONOLAKE_HOME/combined.pem
sudo mv $MONOLAKE_HOME/combined.pem /etc/nginx/
sudo cp -r $MONOLAKE_HOME/benchmark/server/webroot/* /usr/share/nginx/html/
sudo service nginx restart
