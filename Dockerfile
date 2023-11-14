FROM rust:1.68

WORKDIR /usr/src/one_todo

COPY . .

RUN cargo build --release --locked

CMD cargo run --release --locked
