// Copyright 2017 The Grin Developers
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

/// Tests exercising the loading and unloading of plugins, as well as the
/// existence and correct functionality of each plugin function

extern crate cuckoo_sys;
extern crate error;

use std::path::PathBuf;
use std::{thread, time};
use std::time::Instant;

use error::CuckooMinerError;
use cuckoo_sys::PluginLibrary;

static DLL_SUFFIX: &str = ".cuckooplugin";

const TEST_PLUGIN_LIBS_CORE : [&str;3] = [
	"lean_cpu_16",
	"lean_cpu_30",
	"mean_cpu_30",
];

const TEST_PLUGIN_LIBS_OPTIONAL : [&str;1] = [
	"lean_cuda_30",
];

//hashes known to return a solution at cuckoo 30 and 16
static KNOWN_30_HASH:&str = "11c5059b4d4053131323fdfab6a6509d73ef22\
9aedc4073d5995c6edced5a3e6";

static KNOWN_16_HASH:&str = "5f16f104018fc651c00a280ba7a8b48db80b30\
20eed60f393bdcb17d0e646538";

//Helper to convert from hex string
fn from_hex_string(in_str: &str) -> Vec<u8> {
	let mut bytes = Vec::new();
	for i in 0..(in_str.len() / 2) {
		let res = u8::from_str_radix(&in_str[2 * i..2 * i + 2], 16);
		match res {
			Ok(v) => bytes.push(v),
			Err(e) => println!("Problem with hex: {}", e),
		}
	}
	bytes
}

//Helper to load a plugin library
fn load_plugin_lib(plugin:&str) -> Result<PluginLibrary, CuckooMinerError> {
	let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
	d.push(format!("../target/debug/plugins/{}{}", plugin, DLL_SUFFIX).as_str());
	PluginLibrary::new(d.to_str().unwrap())
}

//Helper to load all plugin libraries specified above
fn load_all_plugins() -> Vec<PluginLibrary>{
	let mut plugin_libs:Vec<PluginLibrary> = Vec::new();
	for p in TEST_PLUGIN_LIBS_CORE.into_iter(){
		plugin_libs.push(load_plugin_lib(p).unwrap());
	}
	for p in TEST_PLUGIN_LIBS_OPTIONAL.into_iter(){
		let pl = load_plugin_lib(p);
		if let Ok(p) = pl {
			plugin_libs.push(p);
		}
	}
	plugin_libs
}

//loads and unloads a plugin many times
#[test]
fn plugin_loading(){
	//core plugins should be built on all systems, fail if they don't exist
	for _ in 0..100 {
		for p in TEST_PLUGIN_LIBS_CORE.into_iter() {
			let pl = load_plugin_lib(p).unwrap();
			pl.unload();
		}
	}
	//only test these if they do exist (cuda, etc)
	for _ in 0..100 {
		for p in TEST_PLUGIN_LIBS_OPTIONAL.into_iter() {
			let pl = load_plugin_lib(p);
			if let Err(_) = pl {
				break;
			}
			pl.unwrap().unload();
		}
	}
}

//Loads all plugins at once
#[test]
fn plugin_multiple_loading(){
	let _p=load_all_plugins();
}

//tests cuckoo_init() on all available plugins
//multiple calls to cuckoo init should be fine
#[test]
fn cuckoo_init(){
	let iterations = 100;
	let plugins = load_all_plugins();
	for p in plugins.into_iter() {
		for _ in 0..iterations {
			p.call_cuckoo_init();
		}
	}
}

// Helper to test call_cuckoo_description and return results
// Ensures that all plugins *probably* don't overwrite
// their buffers as they contain an null zero somewhere 
// within the rust-enforced length

fn call_cuckoo_description_tests(pl: &PluginLibrary){
	///Test normal value
	const LENGTH:usize = 256;
	let mut name_bytes:[u8;LENGTH]=[0;LENGTH];
	let mut description_bytes:[u8;LENGTH]=[0;LENGTH];
	let mut name_len=name_bytes.len() as u32;
	let mut desc_len=description_bytes.len() as u32;
	pl.call_cuckoo_description(&mut name_bytes, &mut name_len,
		&mut description_bytes, &mut desc_len);
	let result_name = String::from_utf8(name_bytes.to_vec()).unwrap();
	let result_name_length = result_name.find('\0');
	let result_desc = String::from_utf8(description_bytes.to_vec()).unwrap();
	let result_desc_length = result_desc.find('\0');
	
	//Check name is less than rust-enforced length,
	//if there's no \0 the plugin is likely overwriting the buffer
	println!("Name: **{}**", result_name);
	assert!(result_name.len()>0);
	assert!(result_name_length != None);
	assert!(name_len!=0);
	println!("Length: {}", result_name_length.unwrap());
	println!("Description: **{}**", result_desc);
	assert!(result_desc.len()>0);
	assert!(result_desc_length != None);
	assert!(desc_len!=0);
	println!("Length: {}", result_desc_length.unwrap());

	assert!(result_name.contains("cuckoo"));
	assert!(result_desc.contains("cuckoo"));

	///Test provided buffer too short
	const TOO_SHORT_LENGTH:usize = 16;
	let mut name_bytes:[u8;TOO_SHORT_LENGTH]=[0;TOO_SHORT_LENGTH];
	let mut description_bytes:[u8;TOO_SHORT_LENGTH]=[0;TOO_SHORT_LENGTH];
	let mut name_len=name_bytes.len() as u32;
	let mut desc_len=description_bytes.len() as u32;
	pl.call_cuckoo_description(&mut name_bytes, &mut name_len,
		&mut description_bytes, &mut desc_len);
	assert!(name_len==0);
	assert!(desc_len==0);
}

//tests call_cuckoo_description() on all available plugins
#[test]
fn cuckoo_description(){
	let iterations = 100;
	let plugins = load_all_plugins();
	for p in plugins.into_iter() {
		for _ in 0..iterations {
			call_cuckoo_description_tests(&p);
		}
	}
}

// Helper to test call_cuckoo_parameter_list and return results
// Ensures that all plugins *probably* don't overwrite
// their buffers as they contain an null zero somewhere 
// within the rust-enforced length

fn call_cuckoo_parameter_list_tests(pl: &PluginLibrary){
	///Test normal rust-enforced value
	const LENGTH:usize = 1024;
	let mut param_list_bytes:[u8;LENGTH]=[0;LENGTH];
	let mut param_list_bytes_len=param_list_bytes.len() as u32;
	let ret_val=pl.call_cuckoo_parameter_list(&mut param_list_bytes,
		&mut param_list_bytes_len);
	let result_list = String::from_utf8(param_list_bytes.to_vec()).unwrap();
	let result_list_null_index = result_list.find('\0');
	
	//Check name is less than rust-enforced length,
	//if there's no \0 the plugin is likely overwriting the buffer
	println!("Plugin: {}", pl.lib_full_path);
	assert!(ret_val==0);
	println!("Parameter List: **{}**", result_list);
	assert!(result_list.len()>0);
	assert!(result_list_null_index != None);
	println!("Null Index: {}", result_list_null_index.unwrap());

	//Basic form check... json parsing can be checked higher up
	assert!(result_list.contains("["));

	///Test provided length too short
	///Plugin shouldn't explode as a result
	const TOO_SHORT_LENGTH:usize = 64;
	let mut param_list_bytes:[u8;TOO_SHORT_LENGTH]=[0;TOO_SHORT_LENGTH];
	let mut param_list_bytes_len=param_list_bytes.len() as u32;
	let ret_val=pl.call_cuckoo_parameter_list(&mut param_list_bytes,
		&mut param_list_bytes_len);
	let result_list = String::from_utf8(param_list_bytes.to_vec()).unwrap();
	assert!(ret_val==3);
}

//tests call_cuckoo_parameter_list() on all available plugins
#[test]
fn cuckoo_parameter_list(){
	let iterations = 100;
	let plugins = load_all_plugins();
	for p in plugins.into_iter() {
		for _ in 0..iterations {
			call_cuckoo_parameter_list_tests(&p);
		}
	}
}

// Helper to test call_cuckoo_get_parameter and return results
// Ensures that all plugins *probably* don't overwrite
// their buffers as they contain an null zero somewhere 
// within the rust-enforced length

fn call_cuckoo_get_parameter_tests(pl: &PluginLibrary){
	println!("Plugin: {}", pl.lib_full_path);
	//normal param that should be there
	let name = "NUM_THREADS";
	let mut num_threads:u32 = 0;
	let return_value = pl.call_cuckoo_get_parameter(name.as_bytes(), &mut num_threads);
	assert!(num_threads > 0);
	assert!(return_value == 0);

	//normal param that's not there
	let name = "SANDWICHES";
	let mut num_sandwiches:u32 = 0;
	let return_value = pl.call_cuckoo_get_parameter(name.as_bytes(), &mut num_sandwiches);
	assert!(num_sandwiches == 0);
	assert!(return_value == 1);

	//normal param that's not there and is too long
	let name = "SANDWICHESSANDWICHESSANDWICHESSANDWICHESSANDWICHESSANDWICHESANDWICHESSAES";
	let mut num_sandwiches:u32 = 0;
	let return_value = pl.call_cuckoo_get_parameter(name.as_bytes(), &mut num_sandwiches);
	assert!(num_sandwiches == 0);
	assert!(return_value == 4);
}

//tests call_cuckoo_get_parameter() on all available plugins
#[test]
fn cuckoo_get_parameter(){
	let iterations = 100;
	let plugins = load_all_plugins();
	for p in plugins.into_iter() {
		for _ in 0..iterations {
			call_cuckoo_get_parameter_tests(&p);
		}
	}
}

// Helper to test call_cuckoo_set_parameter and return results
// Ensures that all plugins *probably* don't overwrite
// their buffers as they contain an null zero somewhere 
// within the rust-enforced length

fn call_cuckoo_set_parameter_tests(pl: &PluginLibrary){
	println!("Plugin: {}", pl.lib_full_path);
	//normal param that should be there
	let name = "NUM_THREADS";
	let return_value = pl.call_cuckoo_set_parameter(name.as_bytes(), 16);
	assert!(return_value == 0);

	//param is there, but calling it with a value outside its expected range
	let name = "NUM_THREADS";
	let return_value = pl.call_cuckoo_set_parameter(name.as_bytes(), 99999999);
	assert!(return_value == 2);

	//normal param that's not there
	let name = "SANDWICHES";
	let return_value = pl.call_cuckoo_set_parameter(name.as_bytes(), 8);
	assert!(return_value == 1);

	//normal param that's not there and is too long
	let name = "SANDWICHESSANDWICHESSANDWICHESSANDWICHESSANDWICHESSANDWICHESANDWICHESSAES";
	let return_value = pl.call_cuckoo_set_parameter(name.as_bytes(), 8);
	assert!(return_value == 4);

	//get that one back and check value
	let name = "NUM_THREADS";
	let mut num_threads:u32 = 0;
	let return_value = pl.call_cuckoo_get_parameter(name.as_bytes(), &mut num_threads);
	println!("Num Threads: {}", num_threads);
	assert!(return_value == 0);
	assert!(num_threads == 16);
}

//tests call_cuckoo_get_parameter() on all available plugins
#[test]
fn cuckoo_set_parameter(){
	let iterations = 100;
	let plugins = load_all_plugins();
	for p in plugins.into_iter() {
		for _ in 0..iterations {
			call_cuckoo_set_parameter_tests(&p);
		}
	}
}

// Helper to test cuckoo_call
// at this level, given the time involved we're just going to
// do a sanity check that the same known hashe will indeed give
// a solution consistently across plugin implementations

fn cuckoo_call_tests(pl: &PluginLibrary){
	println!("Plugin: {}", pl.lib_full_path);

	//Known Hash
	let mut header = from_hex_string(KNOWN_30_HASH);
	//or 16, if needed
	if pl.lib_full_path.contains("16") {
		header = from_hex_string(KNOWN_16_HASH);
	}

	let mut solution:[u32; 42] = [0;42];
	let result=pl.call_cuckoo(&header, &mut solution);
	if result==1 {
	  println!("Solution Found!");
	} else {
	  println!("No Solution Found");
		println!("Header {:?}", header);
	}
	assert!(result==1);
}

//tests cuckoo_call() on all available plugins
#[test]
fn cuckoo_call(){
	let iterations = 1;
	let plugins = load_all_plugins();
	for p in plugins.into_iter() {
		for _ in 0..iterations {
			cuckoo_call_tests(&p);
		}
	}
}

// Helper to test call_cuckoo_start_processing
// Starts up queue, lets it spin for a bit, 
// then shuts it down. Should be no segfaults
// and everything cleared up cleanly

fn call_cuckoo_start_processing_tests(pl: &PluginLibrary){
	println!("Plugin: {}", pl.lib_full_path);
	//Just start processing
	let ret_val=pl.call_cuckoo_start_processing();

	let wait_time = time::Duration::from_millis(25);

	thread::sleep(wait_time);
	pl.call_cuckoo_stop_processing();

	//wait for internal processing to finish
	while pl.call_cuckoo_has_processing_stopped()==0{};
	pl.call_cuckoo_reset_processing();

	println!("{}",ret_val);
	assert!(ret_val==0);
}

//tests call_cuckoo_start_processing 
//on all available plugins
#[test]
fn call_cuckoo_start_processing(){
	let iterations = 10;
	let plugins = load_all_plugins();
	for p in plugins.into_iter() {
		for _ in 0..iterations {
			call_cuckoo_start_processing_tests(&p);
		}
	}
}

// Helper to test call_cuckoo_push_to_input_queue

fn call_cuckoo_push_to_input_queue_tests(pl: &PluginLibrary){
	println!("Plugin: {}", pl.lib_full_path);

	//hash too long
	let hash:[u8;42]=[0;42];
	let nonce:[u8;8]=[0;8];
	println!("HASH LEN {}", hash.len());
	let result=pl.call_cuckoo_push_to_input_queue(&hash, &nonce);
	println!("Result: {}",result);
	assert!(result==2);

	//basic push
	let hash:[u8;32]=[0;32];
	let nonce:[u8;8]=[0;8];
	let result=pl.call_cuckoo_push_to_input_queue(&hash, &nonce);
	assert!(result==0);

	//push until queue is full
	for i in 0..10000 {
		let result=pl.call_cuckoo_push_to_input_queue(&hash, &nonce);
		if result==1 {
			break;
		}
		//Should have been full long before now
		assert!(i!=10000);
	}

	//should be full
	let result=pl.call_cuckoo_push_to_input_queue(&hash, &nonce);
	assert!(result==1);

	//only do this on smaller test cuckoo, or we'll be here forever
	if pl.lib_full_path.contains("16"){
		pl.call_cuckoo_start_processing();
		let wait_time = time::Duration::from_millis(100);
		thread::sleep(wait_time);
		pl.call_cuckoo_stop_processing();
		//wait for internal processing to finish
		while pl.call_cuckoo_has_processing_stopped()==0{};
	}

	//Clear queues and reset internal 'should_quit' flag
	pl.call_cuckoo_clear_queues();
	pl.call_cuckoo_reset_processing();
}

//tests call_cuckoo_push_to_input_queue
//on all available plugins
#[test]
fn call_cuckoo_push_to_input_queue(){
	let iterations = 10;
	let plugins = load_all_plugins();
	for p in plugins.into_iter() {
		for _ in 0..iterations {
			call_cuckoo_push_to_input_queue_tests(&p);
		}
	}
}

// Helper to test call_cuckoo_stop_processing
// basically, when a plugin is told to shut down,
// it should immediately stop its processing,
// clean up all alocated memory, and terminate 
// its processing thread. This will check to ensure each plugin 
// does so, and does so within a reasonable time frame 

fn call_cuckoo_stop_processing_tests(pl: &PluginLibrary){
	println!("Plugin: {}", pl.lib_full_path);

	//push anything to input queue
	let hash:[u8;32]=[0;32];
	let nonce:[u8;8]=[0;8];
	let result=pl.call_cuckoo_push_to_input_queue(&hash, &nonce);
	println!("Result: {}", result);
	assert!(result==0);

	//start processing, which should take non-trivial time
	//in most cases
	let ret_val=pl.call_cuckoo_start_processing();
	assert!(ret_val==0);

	//Give it a bit to start up
	let wait_time = time::Duration::from_millis(25);
	thread::sleep(wait_time);

	let start=Instant::now();

	//Now stop
	pl.call_cuckoo_stop_processing();

	//wait for internal processing to finish
	while pl.call_cuckoo_has_processing_stopped()==0{};
	pl.call_cuckoo_reset_processing();

	let elapsed=start.elapsed();
	let elapsed_ms=(elapsed.as_secs() * 1_000) + (elapsed.subsec_nanos() / 1_000_000) as u64;
	println!("Shutdown elapsed_ms: {}",elapsed_ms);

	//will give each plugin half a second for now
	//but give cuda libs a pass for now, as they're hard to stop
	if !pl.lib_full_path.contains("cuda"){
		assert!(elapsed_ms<=500);
	}
}

//tests call_cuckoo_start_processing 
//on all available plugins
#[test]
fn call_cuckoo_stop_processing(){
	let iterations = 5;
	let plugins = load_all_plugins();
	for p in plugins.into_iter() {
		for _ in 0..iterations {
			call_cuckoo_stop_processing_tests(&p);
		}
	}

	let pl = load_plugin_lib("lean_cuda_30").unwrap();
	call_cuckoo_stop_processing_tests(&pl);
}

// Helper to test call_cuckoo_read_from_output_queue
// will basically test that each plugin comes back
// with a known solution in async mode

fn call_cuckoo_read_from_output_queue_tests(pl: &PluginLibrary){
	println!("Plugin: {}", pl.lib_full_path);

	//Known Hash
	let mut header = from_hex_string(KNOWN_30_HASH);
	//or 16, if needed
	if pl.lib_full_path.contains("16") {
		header = from_hex_string(KNOWN_16_HASH);
	}
	//Just zero nonce here, for ID
	let nonce:[u8;8]=[0;8];
	let result=pl.call_cuckoo_push_to_input_queue(&header, &nonce);
	println!("Result: {}", result);
	assert!(result==0);

	//start processing
	let ret_val=pl.call_cuckoo_start_processing();
	assert!(ret_val==0);
	//Record time now, because we don't want to wait forever
	let start=Instant::now();

	//if 2 minutes has elapsed, there's no solution
	let max_time_ms=120000;

	let mut sols:[u32; 42] = [0; 42];
	let mut nonce: [u8; 8] = [0;8];
	loop {
		let found = pl.call_cuckoo_read_from_output_queue(&mut sols, &mut nonce);
		if found == 1 {
			println!("Found solution");
			break;
		}
		let elapsed=start.elapsed();
		let elapsed_ms=(elapsed.as_secs() * 1_000) + (elapsed.subsec_nanos() / 1_000_000) as u64;
		if elapsed_ms > max_time_ms{
			panic!("Known solution not found");
		}
	}
	
	//Now stop
	pl.call_cuckoo_stop_processing();

	//wait for internal processing to finish
	while pl.call_cuckoo_has_processing_stopped()==0{};
	pl.call_cuckoo_reset_processing();
}

//tests call_cuckoo_read_from_output_queue() on all available
//plugins

#[test]
fn call_cuckoo_read_from_output_queue(){
	let iterations = 1;
	let plugins = load_all_plugins();
	for p in plugins.into_iter() {
		for _ in 0..iterations {
			call_cuckoo_read_from_output_queue_tests(&p);
		}
	}
}

// Helper to test call_cuckoo_get_stats and return results
// Ensures that all plugins *probably* don't overwrite
// their buffers as they contain an null zero somewhere 
// within the rust-enforced length

fn call_cuckoo_get_stats_test(pl: &PluginLibrary){
	///Test normal value
	const LENGTH:usize = 1024;
	let mut stat_bytes:[u8;LENGTH]=[0;LENGTH];
	let mut stat_bytes_len=stat_bytes.len() as u32;
	let ret_val=pl.call_cuckoo_get_stats(&mut stat_bytes,
		&mut stat_bytes_len);
	let result_list = String::from_utf8(stat_bytes.to_vec()).unwrap();
	let result_list_null_index = result_list.find('\0');
	
	//Check name is less than rust-enforced length,
	//if there's no \0 the plugin is likely overwriting the buffer
	println!("Plugin: {}", pl.lib_full_path);
	assert!(ret_val==0);
	println!("Stat List: **{}**", result_list);
	assert!(result_list.len()>0);
	assert!(result_list_null_index != None);
	println!("Null Index: {}", result_list_null_index.unwrap());

	//Basic form check... json parsing can be checked higher up
	assert!(result_list.contains("["));
	assert!(result_list.contains("]"));

	//Check buffer too small
	const TOO_SMALL:usize = 50;
	let mut stat_bytes:[u8;TOO_SMALL]=[0;TOO_SMALL];
	let mut stat_bytes_len=stat_bytes.len() as u32;
	let ret_val=pl.call_cuckoo_get_stats(&mut stat_bytes,
		&mut stat_bytes_len);
	
	assert!(ret_val==3);

	//Now start up processing and check values
	//Known Hash
	let mut header = from_hex_string(KNOWN_30_HASH);
	//or 16, if needed
	if pl.lib_full_path.contains("16") {
		header = from_hex_string(KNOWN_16_HASH);
	}
	//Just zero nonce here, for ID
	let nonce:[u8;8]=[0;8];
	let result=pl.call_cuckoo_push_to_input_queue(&header, &nonce);
	println!("Result: {}", result);
	assert!(result==0);

	//start processing
	let ret_val=pl.call_cuckoo_start_processing();
	assert!(ret_val==0);
	//Record time now, because we don't want to wait forever
	let start=Instant::now();

	let wait_time = time::Duration::from_millis(5000);
	thread::sleep(wait_time);

	let ret_val=pl.call_cuckoo_get_stats(&mut stat_bytes,
			&mut stat_bytes_len);
	let result_list = String::from_utf8(stat_bytes.to_vec()).unwrap();
	//let result_list_null_index = result_list.find('\0');
	assert!(ret_val==0);
	
	println!("Stats after starting: {}", result_list);
	

}

//tests call_cuckoo_parameter_list() on all available plugins
#[test]
fn call_cuckoo_get_stats(){
	/*let iterations = 100;
	let plugins = load_all_plugins();
	for p in plugins.into_iter() {
		for _ in 0..iterations {
			call_cuckoo_get_stats_tests(&p);
		}
	}*/
	let pl = load_plugin_lib("lean_cuda_30").unwrap();
	call_cuckoo_get_stats_test(&pl);
	panic!();
}
