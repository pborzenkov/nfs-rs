FROM alpine:edge

RUN apk add --no-cache unfs3 rpcbind

ADD exports /etc/exports
ADD start.sh /start.sh
RUN chmod +x /start.sh
RUN install -d -m 0755 -o 65534 -g 65534 /share

CMD ["/start.sh"]
