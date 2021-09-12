FROM ubuntu:devel

RUN apt-get clean
RUN mv /var/lib/apt/lists /tmp
RUN mkdir -p /var/lib/apt/lists/partial
RUN apt-get clean
RUN apt-get update

RUN apt-get install libgtk-4-dev
