#!/bin/sh

# This script is used to generate the needed certificates when run demo.
# Generate accroading to https://github.com/monoio-rs/monoio-tls/blob/master/example/certs/README.md

mkdir certs
cd certs || exit

openssl genrsa -out rootCA.key 4096
openssl req -x509 -new -nodes -sha512 -days 3650 \
-subj "/C=CN/ST=Shanghai/L=Shanghai/O=Monoio/OU=TLSDemo/CN=monoio-ca" \
-key rootCA.key \
-out rootCA.crt

openssl genrsa -out server.key 4096
openssl req -sha512 -new \
-subj "/C=CN/ST=Shanghai/L=Shanghai/O=Monoio/OU=TLSDemoServer/CN=monoio.rs" \
-key server.key \
-out server.csr

cat > v3.ext <<-EOF
authorityKeyIdentifier=keyid,issuer
basicConstraints=CA:FALSE
keyUsage=digitalSignature, nonRepudiation, keyEncipherment, dataEncipherment
extendedKeyUsage=serverAuth
subjectAltName=@alt_names

[alt_names]
DNS.1=gateway.monoio.rs
EOF

openssl x509 -req -sha512 -days 3650 \
-extfile v3.ext \
-CA rootCA.crt -CAkey rootCA.key -CAcreateserial \
-in server.csr \
-out server.crt

# Convert files
rm rootCA.srl server.csr v3.ext
openssl pkcs8 -topk8 -inform PEM -outform PEM -nocrypt -in server.key -out server.pkcs8
