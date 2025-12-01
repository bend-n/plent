use super::{ownership::Ownership, *};
use tokio::sync::Mutex;

#[derive(Copy, Clone, Debug)]
pub enum Person {
    Role(u64),
    User(u64),
}

#[derive(Copy, Clone, Debug)]
pub struct Repo {
    // delete power
    pub admins: &'static [Person],
    /// power to `scour` and `retag` etc.
    pub chief: u64,
    pub deny_emoji: u64,
    /// clone url: https://bend-n:github_pat_…@github.com/…
    pub auth: &'static str,
    pub name: &'static str,
    ownership: &'static LazyLock<Mutex<Ownership>>,
    // possibly posters?
}

// const EMAILMAP: phf::Map<u64, &str> = phf::phf_map! {
//     332054403160735765u64 => "97122356+Username1321@users.noreply.github.com",
// };

impl std::cmp::PartialEq for Repo {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

use super::schematic::Schem;
use mindus::data::DataWrite;
impl Repo {
    pub fn auth(&self, member: &Member) -> bool {
        OWNER == member.user.id.get()
            || self.chief == member.user.id.get()
            || (self.admins.iter().any(|&x| match x {
                Person::Role(x) => member.roles.contains(&RoleId::new(x)),
                Person::User(x) => member.user.id.get() == x,
            }))
    }

    pub async fn own(&self) -> tokio::sync::MutexGuard<Ownership> {
        self.ownership.lock().await
    }

    pub fn schem(&self, dir: &str, x: MessageId) -> std::io::Result<mindus::Schematic> {
        std::fs::read(self.path(dir, x))
            .map(|x| mindus::Schematic::deserialize(&mut mindus::data::DataRead::new(&x)).unwrap())
    }

    pub fn path(&self, dir: &str, x: MessageId) -> std::path::PathBuf {
        self.repopath()
            .join(dir)
            .join(format!("{:x}.msch", x.get()))
    }

    pub fn gpath(&self, dir: &str, x: MessageId) -> std::path::PathBuf {
        Path::new(dir).join(format!("{:x}.msch", x.get()))
    }

    pub fn has(&self, dir: &str, x: MessageId) -> bool {
        self.path(dir, x).exists()
    }

    pub fn repopath(&self) -> std::path::PathBuf {
        Path::new("repos").join(format!("{}", self.name))
    }

    pub fn remove(&self, dir: &str, x: MessageId) {
        assert!(
            std::process::Command::new("git")
                .current_dir(self.repopath())
                .arg("rm")
                .arg("-q")
                .arg("-f")
                .arg(self.gpath(dir, x))
                .status()
                .unwrap()
                .success()
        );
    }

    pub fn pull(&self) {
        assert!(
            std::process::Command::new("git")
                .current_dir(self.repopath())
                .arg("pull")
                .arg("-q")
                .status()
                .unwrap()
                .success()
        );
    }

    pub fn commit(&self, by: &str, user_id: impl Into<u64>, msg: &str) {
        assert!(
            std::process::Command::new("git")
                .current_dir(self.repopath())
                .args(["commit", "--no-gpg-sign", "-q", "--author"])
                .arg(format!("{by} <@designit>"))
                .arg("-m")
                .arg(msg)
                .status()
                .unwrap()
                .success()
        );
    }

    pub fn push(&self) {
        #[cfg(not(debug_assertions))]
        assert!(
            std::process::Command::new("git")
                .current_dir(self.repopath())
                .arg("push")
                .arg("-q")
                .status()
                .unwrap()
                .success()
        )
    }

    pub fn write(&self, dir: &str, x: MessageId, s: Schem) {
        _ = std::fs::create_dir(self.repopath().join(dir));
        let mut v = DataWrite::default();
        s.serialize(&mut v).unwrap();
        std::fs::write(self.path(dir, x), v.consume()).unwrap();
        self.add();
    }

    pub fn add(&self) {
        assert!(
            std::process::Command::new("git")
                .current_dir(self.repopath())
                .arg("add")
                .arg(".")
                .status()
                .unwrap()
                .success()
        );
    }
}

macro_rules! decl {
    (
        [$($threaded:literal)+ $(,)?];
        $(
            $repo:ident => [
                $(
                    // xu64 => "dirname" : [label, label]
                    $ch:literal => $item:literal : [$($labels: expr),* $(,)?]
                ),*

                $(forum $chf:literal => $itemf:literal),*

            ];
        )+
    ) => {
        use crate::emoji::to_mindustry::named::*;
        pub static THREADED: phf::Set<u64> = phf::phf_set! { $($threaded,)+ };
        pub static SPECIAL: phf::Map<u64, Ch> = phf::phf_map! {
            $($($ch => Ch { d: $item, ty: Type::Basic(&[$($labels,)+]), repo: &$repo },)?)*
        };
        pub static FORUMS: phf::Map<u64, Ch> = phf::phf_map! {
            $($($chf => Ch { d: $itemf, ty: Type::Forum(()), repo: &$repo },)*)+
        };
    };
}
macro_rules! person {
    (&$x:literal) => {
        Person::Role($x)
    };
    ($x:literal) => {
        Person::User($x)
    };
}

macro_rules! repos {
    (
        $($repo_ident:ident => { admins: $admins:expr, chief: $chief:expr, deny_emoji: $deny:expr,} $(,)?),+
    ) => { paste::paste! {
        $(static [<$repo_ident _OWN>]: LazyLock<Mutex<Ownership>> = LazyLock::new(|| Mutex::new(super::ownership::Ownership::new(stringify!($repo_ident))));)+
        $(pub static $repo_ident: Repo = Repo {
                admins: $admins,
                chief: $chief,
                deny_emoji: $deny,
                name: stringify!($repo_ident),
                auth: include_str!(concat!("../../", stringify!($repo_ident), ".auth")),
                ownership: &[<$repo_ident _OWN>],
        };)+
        pub static ALL: &[&Repo] = &[$(&$repo_ident),+];
    }};
}

repos! {
    DESIGN_IT => {
        admins: &[person!(&925676016708489227)],
        chief: 696196765564534825,
        deny_emoji: 1192388789952319499u64,
    },
    ACP => {
        admins: &[person!(&1110439183190863913)],
        chief: 696196765564534825,
        deny_emoji: 1182469319641272370u64,
    },
    MISC => {
        admins: &[person!(&925676016708489227)],
        chief: 705503407179431937,
        deny_emoji: 1192388789952319499u64,
    },
    EVICT => {
        admins: &[person!(&1394078459940175963), person!(&1389145164299243520)],
        chief: 954347786193747968,
        deny_emoji: 1395478597451518085u64,
    }
}
pub const ERE: &str = "[#ff9266][]";
pub const SRP: &str = "[#854aff][]";
pub const L: &str = "check for launch pad";
decl! {
    [1129391545418797147u64];
    DESIGN_IT => [
925721957209636914u64 => "cryofluid" : [CRYOFLUID, CRYOFLUID_MIXER, SRP, L],
925721791475904533u64 => "graphite" : [GRAPHITE, GRAPHITE_PRESS, SRP, L],
925721824556359720u64 => "metaglass" : [METAGLASS, KILN, SRP, L],
925721863525646356u64 => "phase-fabric" : [PHASEFABRIC, PHASE_WEAVER, SRP, L],
927036346869104693u64 => "plastanium" : [PLASTANIUM, PLASTANIUM_COMPRESSOR, SRP, L],
925736419983515688u64 => "pyratite" : [PYRATITE, PYRATITE_MIXER, SRP, L],
925736573037838397u64 => "blast-compound" : [BLASTCOMPOUND, BLAST_MIXER, SRP, L],
927793648417009676u64 => "scrap" : [DISASSEMBLER, SCRAP, SRP, L],
1198556531281637506u64 => "spore-press" : [OIL, SPORE_PRESS, SRP, L],
1200308146460180520u64 => "oil-extractor" : [OIL, OIL_EXTRACTOR, SRP, L],
1200301847387316317u64 => "rtg-gen" : [POWER, RTG_GENERATOR, SRP, L],
1200308292744921088u64 => "cultivator" : [SPOREPOD, CULTIVATOR, SRP, L],
1200305956689547324u64 => "graphite-multipress" : [GRAPHITE, MULTI_PRESS, SRP, L],
1200306409036857384u64 => "silicon-crucible" : [SILICON, SILICON_CRUCIBLE, SRP, L],
1198555991667646464u64 => "coal" : [COAL, COAL_CENTRIFUGE, SRP, L],
925721763856404520u64 => "silicon" : [SILICON, SILICON_SMELTER, SRP, L],
925721930814869524u64 => "surge-alloy" : [SURGEALLOY, SURGE_SMELTER, SRP, L],
1141034314163826879u64 => "defensive-outpost" : ["", SRP, L],
949529149800865862u64 => "drills" : [PRODUCTION, SRP, L],
925729855574794311u64 => "logic-schems" : [MICRO_PROCESSOR, SRP, L],
1185702384194818048u64 => "miscellaneous" : ["…", SRP, L],
1018541701431836803u64 => "combustion-gen" : [POWER, COMBUSTION_GENERATOR, SRP, L],
927480650859184171u64 => "differential-gen" : [POWER, DIFFERENTIAL_GENERATOR, SRP, L],
925719985987403776u64 => "impact-reactor" : [POWER, IMPACT_REACTOR, SRP, L],
949740875817287771u64 => "steam-gen" : [POWER, STEAM_GENERATOR, SRP, L],
926163105694752811u64 => "thorium-reactor" : [POWER, THORIUM_REACTOR, SRP, L],
973234467357458463u64 => "carbide" : [CARBIDE, "", ERE, L],
1198527267933007893u64 => "erekir-defensive-outpost" : ["", ERE, L],
973236445567410186u64 => "fissile-matter" : [FISSILEMATTER, "", ERE, L],
1147887958351945738u64 => "electrolyzer" : [HYDROGEN, OZONE, "", ERE, L],
1202001032503365673u64 => "nitrogen" : [NITROGEN, "", ERE, L],
1202001055349477426u64 => "cyanogen" : [CYANOGEN, "", ERE, L],
1096157669112418454u64 => "mass-driver" : ["…", PLANET, ERE, L],
973234248054104115u64 => "oxide" : [OXIDE, "", ERE, L],
973422874734002216u64 => "erekir-phase" : [PHASEFABRIC, "", ERE, L],
973369188800413787u64 => "ccc" : ["", POWER, ERE, L],
1218453338396430406u64 => "neoplasia-reactor": ["", POWER, ERE, L],
1218453292045172817u64 => "flux-reactor": ["", POWER, ERE, L],
1218452986788053012u64 => "pyrolisis-gen": ["", POWER, ERE, L],
1147722735305367572u64 => "silicon-arc" : [SILICON, SILICON_ARCFURNACE, ERE, L],
974450769967341568u64 => "erekir-surge" : [SURGEALLOY, SURGE_CRUCIBLE, ERE, L],
973241041685737532u64 => "erekir-units" : [UNITS, ERE, L],
1158818171139133490u64 => "unit-core" : [UNITS, CORE_NUCLEUS, SRP, L],
1158818324210274365u64 => "unit-delivery" : [UNITS, FLARE, SRP, L],
1158818598568075365u64 => "unit-raw" : [UNITS, PRODUCTION, SRP, L],
1142181013779398676u64 => "unit-sand" : [UNITS, SAND, SRP, L],
1222270513045438464u64 => "bore": [PRODUCTION, ERE, L],
1226407271978766356u64 => "pulveriser": [PULVERIZER, SAND, SRP, L],
1277138620863742003u64 => "melter": [MELTER, SLAG, SRP, L],
1277138532355543070u64 => "separator": [SEPARATOR, SCRAP, SRP, L]
    ];
MISC => [
forum 1297452357549821972u64 => "s-defensive-outpost",
forum 1297451239449038931u64 => "s-drill-pump",
forum 1297449976015618109u64 => "s-miscellaneous",
forum 1297448333895405588u64 => "s-power",
forum 1297437167647461446u64 => "s-resources",
forum 1297449040438493195u64 => "s-units",
forum 1297463948999790642u64 => "e-bore-pump",
forum 1297460753145659453u64 => "e-defensive-outpost",
forum 1297464596810174508u64 => "e-miscellaneous",
forum 1297463058092003328u64 => "e-power",
forum 1297462381298843680u64 => "e-resources",
forum 1297463616035098654u64 => "e-units"
    ];
ACP => [
        1276759410722738186u64 => "schems": ["plague"]
    ];
EVICT => [
1394340637758722048u64 => "t1" : ["find unit factory"],
1394340696227319808u64 => "t2" : [ADDITIVE_RECONSTRUCTOR],
1394340759297065252u64 => "t3" : [MULTIPLICATIVE_RECONSTRUCTOR],
1394340939685953656u64 => "t4" : [EXPONENTIAL_RECONSTRUCTOR],
1394340961550729237u64 => "t5" : [TETRATIVE_RECONSTRUCTOR],
1394418354634362910u64 => "combustion-gen" : [COMBUSTION_GENERATOR],
1394418605461999769u64 => "differential-gen" : [DIFFERENTIAL_GENERATOR],
1394418707257626795u64 => "impact-reactor" : [IMPACT_REACTOR],
1394418470430707712u64 => "steam-gen" : [STEAM_GENERATOR],
1394418672755282040u64 => "thorium-reactor" : [THORIUM_REACTOR],
1394412929587347556u64 => "coal" : [COAL],
1394081205024063529u64 => "graphite" : [GRAPHITE],
1394081280978583624u64 => "meta" : [METAGLASS],
1394081566950424577u64 => "phase" : [PHASEFABRIC],
1394081445781176493u64 => "plast" : [PLASTANIUM],
1394341640105103400u64 => "resource-mix" : [ROUTER],
1394081152611913918u64 => "silicon" : [SILICON],
1394081607991820368u64 => "surge" : [SURGEALLOY],
1394342304021741672u64 => "miners" : [BLAST_DRILL],
1394342482027745351u64 => "scrap-schematics" : [SCRAP],
1394335945074933901u64 => "cyclone" : [CYCLONE],
1394083007245320244u64 => "fuse" : [FUSE],
1394083227001557034u64 => "hail-scorch" : [HAIL, SCORCH],
1394082982733680722u64 => "scatter": [SCATTER],
1394335981691211806u64 => "swarmer": [SWARMER],
1394414493957881906u64 => "miscellaneous" : [NEOPLASM]
    ];
}

macro_rules! chief {
    ($c:ident) => {{
        let repo = repos::SPECIAL
            .get(&$c.channel_id().get())
            .ok_or(anyhow::anyhow!("not repo"))?
            .repo;
        if repo.chief != $c.author().id.get() && $c.author().id.get() != OWNER {
            poise::send_reply(
                $c,
                poise::CreateReply::default()
                    .content(format!(
                        "access denied. only the chief <@{}> can use this command.",
                        repo.chief,
                    ))
                    .allowed_mentions(CreateAllowedMentions::default().empty_users().empty_roles()),
            )
            .await?;
            return Ok(());
        }
        repo
    }};
}

pub(crate) use chief;
