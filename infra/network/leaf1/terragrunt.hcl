locals {
  hostname      = "leaf1"
  shared_locals = read_terragrunt_config(find_in_parent_folders("locals.hcl")).locals
}

terraform {
  source = find_in_parent_folders("modules/mikrotik")
}

inputs = {
  mikrotik_hostname = "https://${local.shared_locals.nodes[local.hostname].management_ip4}"
  mikrotik_username = get_env("MIKROTIK_USERNAME")
  mikrotik_password = get_env("MIKROTIK_PASSWORD")
  mikrotik_insecure = true

  certificate_common_name = local.shared_locals.nodes[local.hostname].management_ip4
  hostname                = local.hostname
  nodes                   = local.shared_locals.nodes
  ntp_servers             = [local.shared_locals.ntp_server]
  timezone                = local.shared_locals.timezone

  vlans = local.shared_locals.vlans

  extra_routes = {
    ipv4 = {
      nat64_pool = {
        dst_address = "10.64.64.0/24"
        gateway     = "10.0.20.10"
        comment     = "NAT64 pool via netpi"
      }
    }
    ipv6 = {
      nat64 = {
        dst_address = "64:ff9b::/96"
        gateway     = "2601:540:381:150::10"
        comment     = "NAT64 via netpi"
      }
    }
  }

  ethernet_interfaces = {
    "ether1" = {
      comment  = "net-cluster-1"
      untagged = local.shared_locals.vlans.NetCluster.name
    }
    "ether2" = {
      comment  = "lab-control-plane-1"
      untagged = local.shared_locals.vlans.LabCluster.name
    }
    "ether3" = {
      comment  = "lab-control-plane-2"
      untagged = local.shared_locals.vlans.LabCluster.name
    }
    "ether4" = {
      comment  = "lab-control-plane-3"
      untagged = local.shared_locals.vlans.LabCluster.name
    }
    "ether5" = {
      comment  = "titan-r"
      untagged = local.shared_locals.vlans.Trusted.name
    }
    "ether6"  = {}
    "ether7"  = {}
    "ether8"  = {}
    "ether9"  = {}
    "ether10" = {}
    "ether11" = {}
    "ether12" = {
      comment  = "TV"
      untagged = local.shared_locals.vlans.Trusted.name
    }
    "ether13" = {
      comment  = "Management"
      untagged = local.shared_locals.vlans.Management.name
    }
    "ether14" = {
      comment  = "WiFi AP",
      untagged = local.shared_locals.vlans.Trusted.name
    }
    "ether15" = {
      comment     = "MGMT[SELF]"
      bridge_port = false
    }
    "ether16" = {
      comment     = "WAN"
      bridge_port = false
      wan_port    = true
    }
  }
}
