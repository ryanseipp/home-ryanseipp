# =============================================================================
# Device settings
# =============================================================================
variable "hostname" {
  type        = string
  description = "The name to assign to this device."
}

variable "timezone" {
  type        = string
  default     = "America/New_York"
  description = "The timezone to set on the device."
}

variable "ntp_servers" {
  type        = list(string)
  default     = ["time.nist.gov"]
  description = "List of NTP servers to use."
}


# =============================================================================
# Certificate details
# =============================================================================
variable "certificate_common_name" {
  type        = string
  description = "CN for the device certificate."
}

variable "certificate_country" {
  type        = string
  default     = "US"
  description = "Country code for the device certificate."
}

variable "certificate_locality" {
  type        = string
  default     = "PA"
  description = "Locality for the device certificate."
}

variable "certificate_organization" {
  type        = string
  default     = "RYANSEIPP"
  description = "Organization for the device certificate."
}

variable "certificate_unit" {
  type        = string
  default     = "HOME"
  description = "Organizational unit for the device certificate."
}


# =============================================================================
# Bridge settings
# =============================================================================
variable "bridge_name" {
  type        = string
  default     = "bridge"
  description = "Name of the main bridge interface"
}

variable "bridge_comment" {
  type        = string
  default     = ""
  description = "Comment for the bridge interface"
}

variable "bridge_mtu" {
  type        = number
  default     = 1514
  description = "MTU for the bridge interface. If null, defaults to 1514"
}


# =================================================================================================
# VLAN Configuration
# =================================================================================================
variable "vlans" {
  type = map(object({
    name    = string
    vlan_id = number
    mtu     = optional(number, 1500)
    ipv4 = optional(object({
      enabled     = optional(bool, true)
      network     = optional(string)
      cidr_suffix = optional(string)
      gateway     = optional(string)
      dhcp_pool   = optional(list(string))
      dns_servers = optional(list(string))
      static_leases = optional(map(object({
        name = string
        mac  = string
      })))
    }))
    ipv6 = optional(object({
      enabled       = optional(bool, true)
      prefix        = optional(string)
      prefix_length = optional(number, 64)
      dns_servers   = optional(list(string))
    }))
  }))
  default     = {}
  description = "Map of VLANs to configure"
}

locals {
  ipv4_vlans = zipmap([for k, v in var.vlans : k if v.ipv4.enabled == true], [for k, v in var.vlans : v if v.ipv4.enabled == true])
  ipv6_vlans = zipmap([for k, v in var.vlans : k if v.ipv6.enabled == true], [for k, v in var.vlans : v if v.ipv6.enabled == true])
}


# =================================================================================================
# Interface Configuration
# =================================================================================================
variable "ethernet_interfaces" {
  type = map(object({
    comment     = optional(string, "")
    bridge_port = optional(bool, true)
    wan_port    = optional(bool, false)
    l2mtu       = optional(number, 1514) # Layer 2 MTU
    mtu         = optional(number, 1500) # Layer 3 MTU

    # VLAN configurations
    tagged   = optional(list(string)) # list of VLAN names
    untagged = optional(string)       # VLAN name for untagged traffic
  }))
  default     = {}
  description = "Map of ethernet interfaces to configure"
}


# =============================================================================
# BGP Peer Configuration
# =============================================================================
variable "nodes" {
  type = map(object({
    mode             = string
    asn              = number
    loopback_ip4     = string
    loopback_ip6     = string
    management_ip4   = string
    management_iface = optional(string)
    peers = optional(map(object({
      iface = string
      cidrs = object({
        v4 = string
        v6 = string
      })
    })))
  }))
  default     = {}
  description = "Map of all nodes to reference when setting up bgp peers"
}

# =============================================================================
# Static Routes
# =============================================================================
variable "extra_routes" {
  type = object({
    ipv4 = optional(map(object({
      dst_address = string
      gateway     = string
      comment     = optional(string, "")
    })), {})
    ipv6 = optional(map(object({
      dst_address = string
      gateway     = string
      comment     = optional(string, "")
    })), {})
  })
  default = {
    ipv4 = {}
    ipv6 = {}
  }
  description = "Extra static routes to configure on this device"
}

locals {
  node = var.nodes[var.hostname]
}
