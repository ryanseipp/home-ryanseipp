locals {
  timezone   = "America/New_York"
  ntp_server = "time.nist.gov"

  cidrs = {
    "spine1-leaf1" = { v4 = "172.20.0.2/31", v6 = "fd00:cafe:beef:0::3/127" }
    "spine1-leaf2" = { v4 = "172.20.0.6/31", v6 = "fd00:cafe:beef:0::6/127" }
    "spine1-leaf3" = { v4 = "172.20.0.10/31", v6 = "fd00:cafe:beef:0::a/127" }
    "spine1-leaf4" = { v4 = "172.20.0.14/31", v6 = "fd00:cafe:beef:0::e/127" }
    "spine1-leaf5" = { v4 = "172.20.0.18/31", v6 = "fd00:cafe:beef:0::12/127" }

    "spine2-leaf1" = { v4 = "172.20.0.4/31", v6 = "fd00:cafe:beef:0::4/127" }
    "spine2-leaf2" = { v4 = "172.20.0.8/31", v6 = "fd00:cafe:beef:0::8/127" }
    "spine2-leaf3" = { v4 = "172.20.0.12/31", v6 = "fd00:cafe:beef:0::c/127" }
    "spine2-leaf4" = { v4 = "172.20.0.16/31", v6 = "fd00:cafe:beef:0::10/127" }
    "spine2-leaf5" = { v4 = "172.20.0.20/31", v6 = "fd00:cafe:beef:0::14/127" }
  }

  nodes = {
    "spine1" = {
      mode             = "spine"
      asn              = 65001
      loopback_ip4     = "172.20.255.1"
      loopback_ip6     = "fd00:cafe:beef:ffff::1"
      management_ip4   = "192.168.88.10"
      management_iface = "ether1"
      peers = {
        "leaf1" = { iface = "sfp-sfpplus1", cidrs = local.cidrs["spine1-leaf1"] }
        "leaf2" = { iface = "sfp-sfpplus2", cidrs = local.cidrs["spine1-leaf2"] }
        # "leaf3" = { iface = "sfp-sfpplus3", cidrs = local.cidrs["spine1-leaf3"] }
        # "leaf4" = { iface = "sfp-sfpplus4", cidrs = local.cidrs["spine1-leaf4"] }
        # "leaf5" = { iface = "sfp-sfpplus5", cidrs = local.cidrs["spine1-leaf5"] }
      }
    }
    "spine2" = {
      mode             = "spine"
      asn              = 65002
      loopback_ip4     = "172.20.255.2"
      loopback_ip6     = "fd00:cafe:beef:ffff::2"
      management_ip4   = "192.168.88.11"
      management_iface = "ether1"
      peers = {
        "leaf1" = { iface = "sfp-sfpplus1", cidrs = local.cidrs["spine2-leaf1"] }
        "leaf2" = { iface = "sfp-sfpplus2", cidrs = local.cidrs["spine2-leaf2"] }
        # "leaf3" = { iface = "sfp-sfpplus3", cidrs = local.cidrs["spine2-leaf3"] }
        # "leaf4" = { iface = "sfp-sfpplus4", cidrs = local.cidrs["spine2-leaf4"] }
        # "leaf5" = { iface = "sfp-sfpplus5", cidrs = local.cidrs["spine2-leaf5"] }
      }
    }
    "leaf1" = {
      mode             = "leaf"
      asn              = 65101
      loopback_ip4     = "172.20.254.1"
      loopback_ip6     = "fd00:cafe:beef:fffe::1"
      management_ip4   = "192.168.88.50"
      management_iface = "ether15"
      peers = {
        "spine1" = { iface = "sfp-sfpplus1", cidrs = local.cidrs["spine1-leaf1"] }
        "spine2" = { iface = "sfp-sfpplus2", cidrs = local.cidrs["spine2-leaf1"] }
      }
    }
    "leaf2" = {
      mode           = "leaf"
      asn            = 65102
      loopback_ip4   = "172.20.254.2"
      loopback_ip6   = "fd00:cafe:beef:fffe::2"
      management_ip4 = "192.168.88.51"
    }
  }

  vlans = {
    "Trusted" = {
      name    = "Trusted"
      vlan_id = 10
      ipv4 = {
        network     = "10.0.1.0"
        cidr_suffix = "24"
        gateway     = "10.0.1.1"
        dhcp_pool   = ["10.0.1.100-10.0.1.199"]
        dns_servers = ["9.9.9.9", "149.112.112.112"]
      }
      ipv6 = {
        prefix        = "2601:540:37f:e241::"
        prefix_length = 64
        dns_servers   = ["2620:fe::fe", "2620:fe::9"]
      }
    }
    "NetCluster" = {
      name    = "NetCluster"
      vlan_id = 20
      ipv4 = {
        enabled = false
      }
      ipv6 = {
        prefix        = "2601:540:37f:e242::"
        prefix_length = 64
        dns_servers   = []
      }
    }
    "LabCluster" = {
      name    = "LabCluster"
      vlan_id = 30
      ipv4 = {
        enabled = false
      }
      ipv6 = {
        prefix        = "2601:540:37f:e243::"
        prefix_length = 64
        dns_servers   = []
      }
    }
    "Management" = {
      name    = "Management"
      vlan_id = 1000
      ipv4 = {
        network     = "192.168.88.0"
        cidr_suffix = "24"
        gateway     = "192.168.88.1"
        dhcp_pool   = ["192.168.88.100-192.168.88.199"]
        dns_servers = ["9.9.9.9", "149.112.112.112"]
        static_leases = {
          "192.168.88.10" = { name = "spine1", mac = "04:F4:1C:8E:8B:1C" }
          "192.168.88.11" = { name = "spine2", mac = "04:F4:1C:8E:8A:EF" }
          "192.168.88.50" = { name = "leaf1", mac = "04:F4:1C:51:82:C0" }
        }
      }
      ipv6 = { enabled = false }
    }
  }
}
