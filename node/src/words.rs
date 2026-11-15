//! The consent texts that the app and the program both show (TECHNICAL 7).
use watane_net::{NodeKey, Role};

pub fn net_on() -> &'static str {
    "Connect your own machines. This machine will announce itself on your local network and connect to the machines that you pair. It publishes nothing on the internet, and uses no relay and no DHT unless you choose one."
}

pub fn dht_on(node: &NodeKey) -> String {
    format!(
        "Publish this machine's addresses in the Mainline DHT. The DHT is a public network that anyone can read. Anyone who knows this machine's key {} can then see its addresses, this machine's local network addresses included, and when it is online. Your identity and your other machines are not published. A record already published can stay readable for some hours after you turn this off.",
        node.short()
    )
}

pub fn relay_add(url: &str) -> String {
    format!(
        "Use the relay {url}. When your machines cannot reach each other directly, their traffic goes through that server. It cannot read or change what they send, but it sees which of your machines talk, when, and how much. Choose only a relay that you run or trust."
    )
}

pub fn join_asked(node: &NodeKey, role: Role) -> String {
    let role = match role {
        Role::Full => "full",
        Role::Light => "light",
    };
    format!(
        "A machine asks to join as a {role} machine. Its key is {}. Pair it only if this is the key that the machine shows. A full machine keeps a copy of everything that your machines hold; a light machine keeps what it uses. Neither can sign for you.",
        node.short()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> NodeKey {
        NodeKey::from_bytes([7; 32])
    }

    #[test]
    fn ac9_each_text_names_its_argument() {
        let k = key();
        assert!(!net_on().is_empty());
        let dht = dht_on(&k);
        assert!(dht.contains(&k.short()) && dht.contains("local network addresses"));
        assert!(relay_add("http://127.0.0.1:9/").contains("http://127.0.0.1:9/"));
        let full = join_asked(&k, Role::Full);
        let light = join_asked(&k, Role::Light);
        assert!(full.contains(&k.short()) && full.contains("as a full machine"));
        assert!(light.contains(&k.short()) && light.contains("as a light machine"));
    }
}
