use wal::LinkedList;

fn main() {
    let list = LinkedList::new();
    for i in 0..50000 {
        list.push_back(i);
    }
}
