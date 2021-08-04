#![no_main]
#![no_std]
use heapless::Vec;
//use rand_core::*;
//use rand::distributions::Uniform;
extern crate panic_halt;
use riscv_rt::entry;

struct Cluster {
    members: Vec<i8, 50>,
    center: i8,
}

fn kmeans (pts: Vec<i8, 50>) -> Vec<Cluster, 5> {
	let mut clusters: Vec<Cluster, 5> = Vec::new(); 
	let mut x = -100;
	for _ in 0..5 {
		//let x = rand_core::new(0, 32); //not sure about range
		x = x + 50; //probably should be random
		clusters.push(Cluster { members: Vec::new(), center: x});
	}
	let mut mov = 20;
	while mov != 0 { //should probably go till nothing moves instead
		//let _mov = 0;
		for j in &pts {
			let mut min: i32 = 127; //idk big number
			for k in &clusters {
				let mut dist = k.center as i32 - *j as i32;
				if dist < 0 {
					dist = dist * -1;
				}
				if dist < min {
					min = dist;
				}
			}
			for a in &mut clusters {
				let mut dist: i32 = a.center as i32 - *j as i32;
				if dist < 0 {
					dist = dist * -1;
				}
				if dist == min {
					a.members.push(*j);
					break
				}
			}
		}
		for mut b in &mut clusters {
			let mut sum: i32 = 0;
			for c in &b.members {
				sum += *c as i32;
			}
			let avg = sum / b.members.len() as i32;
			/*if avg != b.center {
				mov = mov + 1;
			}*/
			b.center = avg as i8;
			b.members.clear();
		}
		mov = mov - 1;
	}
	clusters
}
#[entry]
fn main () -> ! {
	let mut points: Vec<i8, 50> = Vec::new();
	points.push(32);
points.push(-48);
points.push(-98);
points.push(-37);
points.push(-105);
points.push(103);
points.push(84);
points.push(82);
points.push(6);
points.push(12);
points.push(65);
points.push(115);
points.push(99);
points.push(-69);
points.push(13);
points.push(-63);
points.push(-80);
points.push(-32);
points.push(32);
points.push(-89);
points.push(28);
points.push(12);
points.push(-27);
points.push(6);
points.push(-106);
points.push(-74);
points.push(25);
points.push(-123);
points.push(44);
points.push(-52);
points.push(-107);
points.push(55);
points.push(-69);
points.push(9);
points.push(-103);
points.push(86);
points.push(122);
points.push(73);
points.push(122);
points.push(-37);
points.push(-21);
points.push(69);
points.push(-14);
points.push(89);
points.push(14);
points.push(-104);
points.push(-49);
points.push(92);
points.push(-47);
points.push(97);


	let mut clusts = kmeans(points);
	let mut adr = 0x8400;
	let mut adr2 = 0x8400;
	for _ in 0..20 {
		let y = adr2 as *mut i8;
		unsafe {
			core::ptr::write_volatile(y, 0);
		}
		adr2 = adr2 + 1;
	}
	for i in &mut clusts {
		let ins = adr as *mut i8;
		unsafe {
			core::ptr::write_volatile(ins, i.center);
		}
		adr = adr + 4;
	}
	loop {}
}
