#![feature(test, duration_as_u128)]
extern crate fancy_garbling;

extern crate test;
use std::time::{Duration, SystemTime};

// use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::io::{BufReader, Lines};

use fancy_garbling::high_level::Bundler;
use fancy_garbling::numbers;
use fancy_garbling::garble::garble;

const WEIGHTS_FILE  : &str = "../dinn/weights-and-biases/txt_weights.txt";
const BIASES_FILE   : &str = "../dinn/weights-and-biases/txt_biases.txt";
const IMAGES_FILE   : &str = "../dinn/weights-and-biases/txt_img_test.txt";
const LABELS_FILE   : &str = "../dinn/weights-and-biases/txt_labels.txt";

const TOPOLOGY: [usize; 3] = [256, 30, 10];
const NIMAGES: usize = 10000;
// const NIMAGES: usize = 1000;
const NLAYERS: usize = 2;

pub fn main() {
    let mut run_benches = false;
    let mut run_tests = false;

    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "bench" => run_benches = true,
            "test" => run_tests = true,
            _ => panic!("unknown arg {}", arg),
        }
    }

    let q = numbers::modulus_with_width(10);
    println!("q={}", q);

    let weights: Vec<Vec<Vec<u128>>> = read_weights(q);
    let biases:  Vec<Vec<u128>>      = read_biases(q);
    let images:  Vec<Vec<u128>>      = read_images(q);
    let labels:  Vec<usize>          = read_labels();

    let bun = build_circuit(q, weights);

    let inp_biases0 = bun.encode(&biases[0]);
    let inp_biases1 = bun.encode(&biases[1]);

    if run_benches {
        println!("running garble/eval benchmark");

        let mut garble_time = Duration::new(0,0);
        let ntests = 16;
        for _ in 0..ntests {
            let start = SystemTime::now();
            let (gb,_) = garble(bun.borrow_circ());
            test::black_box(gb);
            garble_time += SystemTime::now().duration_since(start).unwrap();
        }
        garble_time /= ntests;

        let (gb,ev) = garble(bun.borrow_circ());

        let inp_img = bun.encode(&images[0]);
        let mut inp = inp_biases0.clone();
        inp.extend(&inp_biases1);
        inp.extend(inp_img);

        let mut eval_time = Duration::new(0,0);
        for _ in 0..ntests {
            let start = SystemTime::now();
            let res = ev.eval(bun.borrow_circ(), &gb.encode(&inp));
            test::black_box(res);
            eval_time += SystemTime::now().duration_since(start).unwrap();
        }
        eval_time /= ntests;

        println!("garbling took {} ms", garble_time.as_millis());
        println!("eval took {} ms", eval_time.as_millis());
        println!("size: {} ciphertexts", ev.size());

    }

    if run_tests {
        println!("running plaintext accuracy evaluation");

        let mut errors = 0;

        for (img_num, img) in images.iter().enumerate() {
            if img_num % 100 == 0 {
                println!("{}/{} {} errors ({}%)", img_num, NIMAGES, errors, 100.0 * (1.0 - errors as f32 / NIMAGES as f32));
            }

            let inp_img = bun.encode(img);

            let mut inp = inp_biases0.clone();
            inp.extend(&inp_biases1);
            inp.extend(inp_img);

            let raw = bun.borrow_circ().eval(&inp);
            let res = bun.decode(&raw);

            let res: Vec<i32> = res.into_iter().map(|x| from_mod_q(q,x)).collect();

            let mut max_val = i32::min_value();
            let mut winner = 0;
            for i in 0..res.len() {
                if res[i] > max_val {
                    max_val = res[i];
                    winner = i;
                }
            }

            if winner != labels[img_num] {
                errors += 1;
            }
        }

        println!("errors: {}/{}. accuracy: {}%", errors, NIMAGES, 100.0 * (1.0 - errors as f32 / NIMAGES as f32));
    }
}

////////////////////////////////////////////////////////////////////////////////
// circuit creation

fn build_circuit(q: u128, weights: Vec<Vec<Vec<u128>>>) -> Bundler {

    let mut b = Bundler::new();
    let nn_biases = vec![b.inputs(q, TOPOLOGY[1]), b.inputs(q, TOPOLOGY[2])];
    let nn_inputs = b.inputs(q, TOPOLOGY[0]);

    let mut layer_outputs = Vec::new();
    let mut layer_inputs;

    for layer in 0..TOPOLOGY.len()-1 {
        if layer == 0 {
            layer_inputs = nn_inputs.clone();
        } else {
            layer_inputs  = layer_outputs;
            layer_outputs = Vec::new();
        }

        let nin  = TOPOLOGY[layer];
        let nout = TOPOLOGY[layer+1];

        for j in 0..nout {
            let mut x = nn_biases[layer][j];
            for i in 0..nin {
                let y = b.cmul(layer_inputs[i], weights[layer][i][j]);
                x = b.add(x, y);
            }
            layer_outputs.push(x);
        }

        if layer == 0 {
            layer_outputs = layer_outputs.into_iter().map(|x| {
                let ms = vec![128];
                let r = b.sgn(x, &ms);
                b.zero_one_to_one_negative_one(r, q)
            }).collect();
        }
    }

    for out in layer_outputs.into_iter() {
        b.output(out);
    }
    b
}

////////////////////////////////////////////////////////////////////////////////
// boilerplate io stuff

fn get_lines(file: &str) -> Lines<BufReader<File>> {
    let f = File::open(file).expect("file not found");
    let r = BufReader::new(f);
    r.lines()
}

fn read_weights(q: u128) -> Vec<Vec<Vec<u128>>> {
    let mut lines = get_lines(WEIGHTS_FILE);
    let mut weights = Vec::with_capacity(NLAYERS);
    for layer in 0..NLAYERS {
        let nin  = TOPOLOGY[layer];
        let nout = TOPOLOGY[layer+1];
        weights.push(Vec::with_capacity(nin));
        for i in 0..nin {
            weights[layer].push(Vec::with_capacity(nout));
            for _ in 0..nout {
                let l = lines.next().expect("no more lines").expect("couldnt read a line");
                let w = l.parse().expect("couldnt parse");
                weights[layer][i].push(to_mod_q(q, w));
            }
        }
    }
    weights
}

fn read_biases(q: u128) -> Vec<Vec<u128>> {
    let mut lines = get_lines(BIASES_FILE);
    let mut biases = Vec::with_capacity(NLAYERS);
    for layer in 0..NLAYERS {
        let nout = TOPOLOGY[layer+1];
        biases.push(Vec::with_capacity(nout));
        for _ in 0..nout {
            let l = lines.next().expect("no more lines").expect("couldnt read a line");
            let w = l.parse().expect("couldnt parse");
            biases[layer].push(to_mod_q(q,w));
        }
    }
    biases
}

fn read_images(q: u128) -> Vec<Vec<u128>> {
    let mut lines = get_lines(IMAGES_FILE);
    let mut images = Vec::with_capacity(NIMAGES);
    for i in 0..NIMAGES {
        images.push(Vec::new());
        for _ in 0..TOPOLOGY[0] {
            let l = lines.next().expect("no more lines").expect("couldnt read a line");
            let w = l.parse().expect("couldnt parse");
            images[i].push(to_mod_q(q,w));
        }
    }
    images
}

fn read_labels() -> Vec<usize> {
    get_lines(LABELS_FILE)
        .map(|line| line.expect("couldnt read").parse().expect("couldnt parse"))
        .collect()
}

fn to_mod_q(q: u128, x: i16) -> u128 {
    ((q as i128 + x as i128) % q as i128) as u128
}

fn from_mod_q(q: u128, x: u128) -> i32 {
    if x > q/2 {
        (q as i128 / 2 - x as i128) as i32
    } else {
        x as i32
    }
}