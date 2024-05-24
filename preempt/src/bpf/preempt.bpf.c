#include "vmlinux.h"
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_core_read.h>
#include <bpf/bpf_tracing.h>
char LICENSE[] SEC("license") = "Dual BSD/GPL";


struct preempt_event{
	pid_t prev_pid;
	pid_t next_pid;
	unsigned long long duration;
	char comm[16];
};

struct {
    __uint(type,BPF_MAP_TYPE_HASH);
    __uint(max_entries, 4096);
    __type(key, pid_t);
    __type(value, u64);
} preemptTime SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_RINGBUF);
	__uint(max_entries, 256 * 1024);
} rb SEC(".maps");


SEC("tp_btf/sched_switch")
int BPF_PROG(sched_switch, bool preempt, struct task_struct *prev, struct task_struct *next){
    int *pid;
    u64 start_time = bpf_ktime_get_ns();
    pid_t prev_pid = BPF_CORE_READ(prev,pid);
    if(preempt){
        bpf_map_update_elem(&preemptTime, &prev_pid, &start_time, BPF_ANY);
    }
    return 0;
}


SEC("kprobe/finish_task_switch") 
int BPF_KPROBE(finish_task_switch,struct task_struct *prev){
    u64 end_time=bpf_ktime_get_ns();
    pid_t pid=BPF_CORE_READ(prev,pid);
    struct preempt_event *e;
    u64 *val;
    val= bpf_map_lookup_elem(&preemptTime,&pid);
    if(val){
        u64 delta=end_time-*val;;
        e = bpf_ringbuf_reserve(&rb, sizeof(*e), 0);
	    if (!e)
		    return 0;
        e->prev_pid=pid;
        e->next_pid=bpf_get_current_pid_tgid()>>32;
        e->duration=delta;
        bpf_get_current_comm(&e->comm, sizeof(e->comm));
        bpf_ringbuf_submit(e, 0);
        bpf_map_delete_elem(&preemptTime, &pid);    
    }
    return 0;
}

