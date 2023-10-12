FROM rust:1.67

WORKDIR /usr/src/one_todo

COPY . .

RUN cargo build --release --locked

RUN cargo run --release --locked
