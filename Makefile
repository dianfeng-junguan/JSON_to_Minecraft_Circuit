all:
	cargo run -- -i testch.json -o testch.lithematic -l lib --graph-json -g
sim:
	cargo run -- -i lib/and.json -o and_sim.json -l lib -s sim.json