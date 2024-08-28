# stop envoy proxy service
sudo kill -15 $(ps aux | grep 'envoy' | awk '{print $2}')
